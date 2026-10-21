//! The LibreOfficeKit engine: the document kinds LibreOffice reads.
//!
//! Opening a document takes long enough to be felt, so the kernel is
//! asked away from the interface thread. The window gets a placeholder
//! right away, and the placeholder shows the document itself once the
//! kernel is through.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::bail;
use ely_lok::{Doc, LokView, Worker};
use futures::channel::oneshot;
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div,
};

use crate::engine::{CommandSpec, DocId, EngineKind, Opened, Source};
use crate::EngineDyn;

/// The engine for the document kinds LibreOffice reads.
pub struct LokEngine {
    kind: EngineKind,
    worker: Arc<Mutex<Worker>>,
    documents: HashMap<DocId, LokDoc>,
    next: DocId,
}

/// What the engine knows about an open document.
struct LokDoc {
    path: PathBuf,
    /// The format it is written back in: the extension it came with.
    format: String,
    /// The placeholder that will show the document; it holds what the
    /// kernel brought once the load is through.
    placeholder: Entity<Loading>,
}

/// What a finished load brings.
struct Ready {
    doc: Doc,
    laid: ely_lok::Layout,
    mark: String,
}

/// The view shown while a document is still loading.
pub struct Loading {
    name: SharedString,
    /// The document itself, once it is there.
    inner: Option<Entity<LokView>>,
    /// Why the document did not open, when it did not.
    reason: Option<SharedString>,
    /// The document in the kernel, once the kernel has it.
    doc: Option<Doc>,
    /// The kernel's history mark as it was when the document was opened.
    mark: String,
}

impl Render for Loading {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match (&self.inner, &self.reason) {
            (Some(view), _) => view.clone().into_any_element(),
            (None, Some(reason)) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_sm().child(reason.clone()))
                .into_any_element(),
            (None, None) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_sm().child(format!("正在打开 {} …", self.name)))
                .into_any_element(),
        }
    }
}

impl LokEngine {
    /// The engine for a kind, over a worker that already runs.
    pub fn new(kind: EngineKind, worker: Arc<Mutex<Worker>>) -> Self {
        Self {
            kind,
            worker,
            documents: HashMap::new(),
            next: 0,
        }
    }

    /// The kinds this engine covers, one engine each.
    pub fn kinds() -> [EngineKind; 3] {
        [
            EngineKind {
                id: "document",
                extensions: &["docx", "odt", "doc", "rtf"],
                default_extension: "docx",
            },
            EngineKind {
                id: "spreadsheet",
                extensions: &["xlsx", "ods", "xls", "csv", "tsv"],
                default_extension: "xlsx",
            },
            EngineKind {
                id: "presentation",
                extensions: &["pptx", "odp", "ppt"],
                default_extension: "pptx",
            },
        ]
    }
}

impl EngineDyn for LokEngine {
    fn kind(&self) -> EngineKind {
        self.kind
    }

    fn open(&mut self, source: Source, cx: &mut App) -> anyhow::Result<Opened> {
        let Source::File(path) = source else {
            bail!("a document must come from a file");
        };
        let format = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .ok_or_else(|| anyhow::anyhow!("the file has no extension"))?;
        let url = ely_lok::file_url(&path);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        // The kernel loads and measures the document away from the window.
        let (sender, receiver) = oneshot::channel();
        let worker = Arc::clone(&self.worker);
        cx.background_executor()
            .spawn(async move {
                let found = worker
                    .lock()
                    .ok()
                    .and_then(|mut worker| {
                        let doc = worker.unlock(&url, "").ok()?;
                        let laid = ely_lok::layout(&mut worker, &doc).ok()?;
                        let mark = worker.values(&doc, ".uno:Undo").ok()?;
                        Some(Ready { doc, laid, mark })
                    });
                let _ = sender.send(found);
            })
            .detach();

        // The window shows a placeholder right away.
        let shown = name.clone();
        let placeholder = cx.new(|_| Loading {
            name: shown.into(),
            inner: None,
            reason: None,
            doc: None,
            mark: String::new(),
        });

        // When the kernel is through, the placeholder shows the document.
        let worker = Arc::clone(&self.worker);
        let place = placeholder.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            let found = receiver.await.unwrap_or(None);
            cx.update(|app| match found {
                Some(found) => {
                    let view = LokView::open(worker, found.doc.clone(), found.laid, app);
                    place.update(app, |loading, cx| {
                        loading.inner = Some(view);
                        loading.doc = Some(found.doc);
                        loading.mark = found.mark;
                        cx.notify();
                    });
                }
                None => {
                    log::error!("lok: the document did not open");
                    place.update(app, |loading, cx| {
                        loading.reason = Some("这个文档没有打开。".into());
                        cx.notify();
                    });
                }
            });
        })
        .detach();

        let id = self.next;
        self.next += 1;
        self.documents.insert(
            id,
            LokDoc {
                path: path.clone(),
                format,
                placeholder: placeholder.clone(),
            },
        );
        Ok(Opened {
            id,
            view: placeholder.clone().into(),
            name: name.into(),
        })
    }

    fn commands(&self, _id: DocId) -> Vec<CommandSpec> {
        // The commands of a document come from the kernel itself; the shell
        // asks for them when it builds its menus.
        Vec::new()
    }

    fn text(&self, _id: DocId, _cx: &App) -> Option<String> {
        // The kernel's text is not read out; the shell saves through the
        // kernel, so it has no use for the text.
        None
    }

    fn modified(&self, id: DocId, cx: &App) -> bool {
        let Some(document) = self.documents.get(&id) else {
            return false;
        };
        let loading = document.placeholder.read(cx);
        let Some(doc) = loading.doc.as_ref() else {
            return false;
        };
        let Ok(mut worker) = self.worker.lock() else {
            return false;
        };
        // The kernel's history mark moves when the document changes.
        match worker.values(doc, ".uno:Undo") {
            Ok(mark) => mark != loading.mark,
            Err(_) => false,
        }
    }

    fn path(&self, id: DocId) -> Option<PathBuf> {
        self.documents.get(&id).map(|document| document.path.clone())
    }

    fn rename(&mut self, id: DocId, path: PathBuf) {
        if let Some(document) = self.documents.get_mut(&id) {
            if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
                document.format = extension.to_ascii_lowercase();
            }
            document.path = path;
        }
    }

    fn write(&mut self, id: DocId, path: &Path, cx: &mut App) -> anyhow::Result<()> {
        let Some(document) = self.documents.get(&id) else {
            bail!("the document is not open");
        };
        let doc = document
            .placeholder
            .read(cx)
            .doc
            .clone()
            .ok_or_else(|| anyhow::anyhow!("the document is still loading"))?;
        let format = document.format.clone();
        let url = ely_lok::file_url(path);
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow::anyhow!("the document engine is not usable"))?;
        doc.save_owned(&mut worker, &url, &format)?;
        let mark = worker.values(&doc, ".uno:Undo")?;
        let placeholder = document.placeholder.clone();
        placeholder.update(cx, |loading, _| loading.mark = mark);
        if let Some(document) = self.documents.get_mut(&id) {
            document.path = path.to_path_buf();
        }
        Ok(())
    }

    fn encoding(&self, _id: DocId) -> Option<crate::Encoding> {
        None
    }

    fn close(&mut self, id: DocId, cx: &App) {
        if let Some(document) = self.documents.remove(&id) {
            // The document may still be loading; then the kernel has none.
            let doc = document.placeholder.read(cx).doc.clone();
            if let Some(doc) = doc {
                if let Ok(mut worker) = self.worker.lock() {
                    let _ = worker.close_doc(doc);
                }
            }
        }
    }

    fn is_loading(&self, id: DocId, cx: &App) -> bool {
        self.documents
            .get(&id)
            .map(|document| document.placeholder.read(cx).doc.is_none())
            .unwrap_or(false)
    }
}
