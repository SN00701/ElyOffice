//! The PDF engine: the PDFium kernel reads and draws the document.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, img,
};

use crate::engine::{CommandSpec, DocId, EngineKind, Opened, Source};
use crate::pdfium::{Document, Pdfium};
use crate::EngineDyn;

/// The engine for PDF documents.
pub struct PdfEngine {
    pdfium: Option<Rc<Pdfium>>,
    documents: HashMap<DocId, PdfDocument>,
    next: DocId,
}

/// What the engine knows about an open document.
struct PdfDocument {
    path: PathBuf,
    pages: usize,
    /// The folder the drawn pages are written to.
    work: PathBuf,
}

/// The view of a PDF: its pages, drawn when they are needed.
pub struct PdfView {
    state: Rc<PdfState>,
}

/// What the view needs to draw a page.
struct PdfState {
    pdfium: Rc<Pdfium>,
    document: Document,
    path: PathBuf,
    pages: Vec<(f32, f32)>,
    work: PathBuf,
    drawn: RefCell<HashMap<usize, Arc<gpui::Image>>>,
    /// How many pages are drawn; the rest come when the reader asks.
    shown: Cell<usize>,
}

impl PdfEngine {
    /// The engine for PDF documents.
    pub fn new() -> Self {
        Self {
            pdfium: None,
            documents: HashMap::new(),
            next: 0,
        }
    }

    /// The kind this engine is for.
    pub fn kind(&self) -> EngineKind {
        EngineKind {
            id: "pdf",
            extensions: &["pdf"],
            default_extension: "pdf",
        }
    }

    /// How many pages the engine has open.
    pub fn pages(&self, id: DocId) -> usize {
        self.documents.get(&id).map(|document| document.pages).unwrap_or(0)
    }

    /// The kernel, loaded when it is first needed.
    fn kernel(&mut self) -> anyhow::Result<Rc<Pdfium>> {
        if let Some(pdfium) = &self.pdfium {
            return Ok(pdfium.clone());
        }
        let pdfium = Rc::new(Pdfium::load()?);
        self.pdfium = Some(pdfium.clone());
        Ok(pdfium)
    }
}

impl EngineDyn for PdfEngine {
    fn kind(&self) -> EngineKind {
        PdfEngine::kind(self)
    }

    fn open(&mut self, source: Source, cx: &mut App) -> anyhow::Result<Opened> {
        let Source::File(path) = source else {
            anyhow::bail!("a PDF must come from a file");
        };
        let pdfium = self.kernel()?;
        let document = pdfium
            .open(&path)
            .ok_or_else(|| anyhow::anyhow!("the PDF could not be read"))?;
        let count = pdfium.pages(document).max(0) as usize;
        let mut pages = Vec::with_capacity(count);
        for index in 0..count {
            let page = pdfium.page(document, index as i32);
            match page {
                Some(page) => {
                    pages.push(pdfium.size(page));
                    pdfium.close_page(page);
                }
                None => pages.push((595.0, 842.0)),
            }
        }
        let work = crate::work_dir("pdf");
        std::fs::create_dir_all(&work).ok();
        let state = Rc::new(PdfState {
            pdfium: pdfium.clone(),
            document,
            path: path.clone(),
            pages,
            work,
            drawn: RefCell::new(HashMap::new()),
            shown: Cell::new(FIRST_PAGES),
        });
        let id = self.next;
        self.next += 1;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "document.pdf".to_string());
        self.documents.insert(
            id,
            PdfDocument {
                path,
                pages: count,
                work: state.work.clone(),
            },
        );
        let view = cx.new(|cx| PdfView { state });
        Ok(Opened {
            id,
            view: view.into(),
            name: name.into(),
        })
    }

    fn commands(&self, _id: DocId) -> Vec<CommandSpec> {
        Vec::new()
    }

    fn text(&self, id: DocId, _cx: &App) -> Option<String> {
        let Some(document) = self.documents.get(&id) else {
            return None;
        };
        let Some(pdfium) = &self.pdfium else {
            return None;
        };
        // The text of every page, read through a handle of its own.
        let Some(handle) = pdfium.open(&document.path) else {
            return None;
        };
        let mut text = String::new();
        for index in 0..pdfium.pages(handle) {
            if let Some(page) = pdfium.page(handle, index) {
                text.push_str(&pdfium.text(page));
                text.push('\n');
                pdfium.close_page(page);
            }
        }
        pdfium.close_document(handle);
        Some(text)
    }

    fn modified(&self, _id: DocId, _cx: &App) -> bool {
        // The window shows the PDF; it does not change it yet.
        false
    }

    fn path(&self, id: DocId) -> Option<PathBuf> {
        self.documents.get(&id).map(|document| document.path.clone())
    }

    fn rename(&mut self, id: DocId, path: PathBuf) {
        if let Some(document) = self.documents.get_mut(&id) {
            document.path = path;
        }
    }

    fn write(&mut self, _id: DocId, _path: &Path, _cx: &App) -> anyhow::Result<()> {
        anyhow::bail!("the window shows PDFs but does not change them yet")
    }

    fn encoding(&self, _id: DocId) -> Option<crate::Encoding> {
        None
    }

    fn close(&mut self, id: DocId) {
        if let Some(document) = self.documents.remove(&id) {
            let _ = std::fs::remove_dir_all(&document.work);
        }
    }
}

impl PdfState {
    /// Draws a page, and says what the picture is.
    fn page_image(&self, index: usize) -> Option<Arc<gpui::Image>> {
        if let Some(image) = self.drawn.borrow().get(&index) {
            return Some(image.clone());
        }
        let (width, height) = *self.pages.get(index)?;
        // A point is a pixel at 96 dots per inch; the picture is twice that.
        let scale = 2.0;
        let pixels = (width * scale).round().max(1.0) as i32;
        let lines = (height * scale).round().max(1.0) as i32;
        let page = self.pdfium.page(self.document, index as i32)?;
        let buffer = self.pdfium.render(page, pixels, lines);
        self.pdfium.close_page(page);
        let buffer = buffer?;
        // The kernel draws BGRA; a picture wants RGBA.
        let mut rgba = vec![0u8; (pixels * lines * 4) as usize];
        for (target, source) in rgba.chunks_exact_mut(4).zip(buffer.chunks_exact(4)) {
            target[0] = source[2];
            target[1] = source[1];
            target[2] = source[0];
            target[3] = 255;
        }
        let picture = image::RgbaImage::from_raw(pixels as u32, lines as u32, rgba)?;
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bytes, image::ImageFormat::Png)
            .ok()?;
        let image = Arc::new(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            bytes.into_inner(),
        ));
        self.drawn.borrow_mut().insert(index, image.clone());
        Some(image)
    }
}

impl Render for PdfView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let count = state.pages.len();
        // Only the pages the reader has asked for are drawn; a thousand-page
        // document does not cost a thousand pictures at the first frame.
        let shown = state.shown.get().min(count);
        let mut items = Vec::new();
        for index in 0..shown {
            let picture = state.page_image(index);
            let number = index + 1;
            let body: gpui::AnyElement = match picture {
                Some(image) => img(gpui::ImageSource::Image(image))
                    .w_full()
                    .into_any_element(),
                None => div().h_32().child("这一页还没有画出来").into_any_element(),
            };
            items.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xs().child(format!("{number}")))
                    .child(body)
                    .into_any_element(),
            );
        }
        if shown < count {
            let this = cx.entity();
            let state = state.clone();
            items.push(
                div()
                    .child(
                        ely_gpui_component::buttons::Button::new(
                            "pdf-more",
                            format!("再显示 {MORE_PAGES} 页（共 {count} 页）"),
                        )
                        .on_click(move |_, _, cx| {
                            state.shown.set(state.shown.get() + MORE_PAGES);
                            this.update(cx, |view, cx| cx.notify());
                        }),
                    )
                    .into_any_element(),
            );
        }
        let mut body = div().size_full().p_3().child(
            ely_gpui_component::layout::ScrollArea::new("pdf-scroll")
                .vertical()
                .child(div().flex().flex_col().gap_3().children(items)),
        );
        if count == 0 {
            body = body.child(div().text_sm().child("这个 PDF 没有页面"));
        }
        body
    }
}

/// How many pages are drawn at the first frame.
const FIRST_PAGES: usize = 8;
/// How many more a click brings.
const MORE_PAGES: usize = 8;

/// How many pages are drawn at once.
const MAX_PAGES: usize = 100;
