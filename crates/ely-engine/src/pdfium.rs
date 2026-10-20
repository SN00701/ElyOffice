//! The PDFium kernel, reached by its own C interface.
//!
//! PDFium is BSD-3; the binding here is written for this project, because
//! the ready-made Rust wrapper (`pdfium-rs`) is GPL and would change the
//! licence of the whole program. The library is loaded at run time, so a
//! build without it still starts and says what is missing.

use std::ffi::c_void;
use std::path::{Path, PathBuf};

use anyhow::bail;

#[cfg(windows)]
use windows_sys::Win32::Foundation::FreeLibrary;
#[cfg(windows)]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

/// A document the kernel holds.
pub type Document = *mut c_void;
/// A page of a document.
pub type Page = *mut c_void;
/// A bitmap the kernel draws into.
pub type Bitmap = *mut c_void;
/// A page's text.
pub type TextPage = *mut c_void;

/// The functions this project uses. Each is checked when the library is
/// loaded, so a call may take it for granted.
#[allow(clippy::type_complexity)]
struct Api {
    init: Option<unsafe extern "C" fn()>,
    destroy: Option<unsafe extern "C" fn()>,
    load_document: Option<unsafe extern "C" fn(*const u8, *const u8) -> Document>,
    close_document: Option<unsafe extern "C" fn(Document)>,
    get_page_count: Option<unsafe extern "C" fn(Document) -> i32>,
    load_page: Option<unsafe extern "C" fn(Document, i32) -> Page>,
    close_page: Option<unsafe extern "C" fn(Page)>,
    page_width: Option<unsafe extern "C" fn(Page) -> f32>,
    page_height: Option<unsafe extern "C" fn(Page) -> f32>,
    bitmap_create: Option<unsafe extern "C" fn(i32, i32, i32) -> Bitmap>,
    bitmap_fill: Option<unsafe extern "C" fn(Bitmap, i32, i32, i32, i32, u32) -> i32>,
    bitmap_buffer: Option<unsafe extern "C" fn(Bitmap) -> *mut c_void>,
    bitmap_destroy: Option<unsafe extern "C" fn(Bitmap)>,
    render_page: Option<unsafe extern "C" fn(Bitmap, Page, i32, i32, i32, i32, i32, i32)>,
    text_load_page: Option<unsafe extern "C" fn(Page) -> TextPage>,
    text_count_chars: Option<unsafe extern "C" fn(TextPage) -> i32>,
    text_get_text: Option<unsafe extern "C" fn(TextPage, i32, i32, *mut u16) -> i32>,
    text_close_page: Option<unsafe extern "C" fn(TextPage)>,
}

impl Api {
    /// A table with nothing in it; every entry is filled at load time.
    fn empty() -> Self {
        Self {
            init: None,
            destroy: None,
            load_document: None,
            close_document: None,
            get_page_count: None,
            load_page: None,
            close_page: None,
            page_width: None,
            page_height: None,
            bitmap_create: None,
            bitmap_fill: None,
            bitmap_buffer: None,
            bitmap_destroy: None,
            render_page: None,
            text_load_page: None,
            text_count_chars: None,
            text_get_text: None,
            text_close_page: None,
        }
    }
}

/// Calls one of the kernel's functions.
macro_rules! call {
    ($self:ident, $name:ident $(, $argument:expr)* $(,)?) => {
        // SAFETY: the symbol was checked when the library was loaded.
        unsafe {
            ($self
                .api
                .$name
                .expect("the kernel's function was checked when it was loaded"))($($argument),*)
        }
    };
}

/// The kernel, loaded and ready.
pub struct Pdfium {
    api: Api,
    #[cfg(windows)]
    library: windows_sys::Win32::Foundation::HMODULE,
}

impl Pdfium {
    /// Loads the kernel from the usual places.
    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = find_library() else {
            bail!("the PDFium library was not found; set ELY_PDFIUM_DIR");
        };
        Self::load_from(&path)
    }

    /// Loads the kernel from one file.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        #[cfg(windows)]
        {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: the path is a null-terminated wide string that outlives the call.
            let library = unsafe { LoadLibraryW(wide.as_ptr()) };
            if library.is_null() {
                bail!("the PDFium library at {} could not be loaded", path.display());
            }
            let mut pdfium = Self {
                api: Api::empty(),
                library,
            };
            macro_rules! symbol {
                ($name:literal, $field:ident) => {{
                    // SAFETY: the name is a null-terminated ASCII string.
                    let symbol = unsafe {
                        GetProcAddress(library, concat!($name, "\0").as_ptr() as *const u8)
                    };
                    let Some(symbol) = symbol else {
                        bail!("the PDFium library has no {}", $name);
                    };
                    // SAFETY: the symbol has the signature the header gives it.
                    pdfium.api.$field = Some(unsafe { std::mem::transmute(symbol) });
                }};
            }
            symbol!("FPDF_InitLibrary", init);
            symbol!("FPDF_DestroyLibrary", destroy);
            symbol!("FPDF_LoadDocument", load_document);
            symbol!("FPDF_CloseDocument", close_document);
            symbol!("FPDF_GetPageCount", get_page_count);
            symbol!("FPDF_LoadPage", load_page);
            symbol!("FPDF_ClosePage", close_page);
            symbol!("FPDF_GetPageWidthF", page_width);
            symbol!("FPDF_GetPageHeightF", page_height);
            symbol!("FPDFBitmap_Create", bitmap_create);
            symbol!("FPDFBitmap_FillRect", bitmap_fill);
            symbol!("FPDFBitmap_GetBuffer", bitmap_buffer);
            symbol!("FPDFBitmap_Destroy", bitmap_destroy);
            symbol!("FPDF_RenderPageBitmap", render_page);
            symbol!("FPDFText_LoadPage", text_load_page);
            symbol!("FPDFText_CountChars", text_count_chars);
            symbol!("FPDFText_GetText", text_get_text);
            symbol!("FPDFText_ClosePage", text_close_page);
            call!(pdfium, init);
            Ok(pdfium)
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            bail!("the PDFium library is only loaded on Windows")
        }
    }

    /// Opens a document.
    pub fn open(&self, path: &Path) -> Option<Document> {
        // The kernel takes the path as UTF-8, not as a wide string.
        let mut bytes = path.to_string_lossy().as_bytes().to_vec();
        bytes.push(0);
        let document = call!(self, load_document, bytes.as_ptr(), std::ptr::null());
        if document.is_null() {
            None
        } else {
            Some(document)
        }
    }

    /// How many pages a document has.
    pub fn pages(&self, document: Document) -> i32 {
        call!(self, get_page_count, document)
    }

    /// Opens one page.
    pub fn page(&self, document: Document, index: i32) -> Option<Page> {
        let page = call!(self, load_page, document, index);
        if page.is_null() { None } else { Some(page) }
    }

    /// Lets a page go.
    pub fn close_page(&self, page: Page) {
        call!(self, close_page, page);
    }

    /// Lets a document go.
    pub fn close_document(&self, document: Document) {
        call!(self, close_document, document);
    }

    /// The page's size in points.
    pub fn size(&self, page: Page) -> (f32, f32) {
        (call!(self, page_width, page), call!(self, page_height, page))
    }

    /// The page's text.
    pub fn text(&self, page: Page) -> String {
        let text_page = call!(self, text_load_page, page);
        if text_page.is_null() {
            return String::new();
        }
        let count = call!(self, text_count_chars, text_page).max(0) as usize;
        let mut wide = vec![0u16; count + 1];
        call!(self, text_get_text, text_page, 0, count as i32, wide.as_mut_ptr());
        call!(self, text_close_page, text_page);
        String::from_utf16_lossy(&wide)
    }

    /// Draws a page into a buffer of BGRA pixels.
    pub fn render(&self, page: Page, width: i32, height: i32) -> Option<Vec<u8>> {
        let bitmap = call!(self, bitmap_create, width, height, 0);
        if bitmap.is_null() {
            return None;
        }
        // White, so a page without a background is not black.
        call!(self, bitmap_fill, bitmap, 0, 0, width, height, 0xFFFF_FFFF);
        call!(self, render_page, bitmap, page, 0, 0, width, height, 0, 0);
        let buffer = call!(self, bitmap_buffer, bitmap);
        // SAFETY: the kernel gave a buffer of the size we asked for.
        let bytes = unsafe {
            std::slice::from_raw_parts(buffer as *const u8, (width * height * 4) as usize)
        };
        let mut pixels = bytes.to_vec();
        call!(self, bitmap_destroy, bitmap);
        Some(pixels)
    }
}

impl Drop for Pdfium {
    fn drop(&mut self) {
        call!(self, destroy);
        #[cfg(windows)]
        unsafe {
            FreeLibrary(self.library);
        }
    }
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Where the kernel may be.
fn find_library() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("ELY_PDFIUM_DIR") {
        let path = PathBuf::from(directory).join("pdfium.dll");
        if path.exists() {
            return Some(path);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(folder) = exe.parent() {
            candidates.push(folder.join("pdfium.dll"));
            candidates.push(folder.join("engines/pdfium/pdfium.dll"));
        }
    }
    candidates.push(PathBuf::from("engines/pdfium/pdfium.dll"));
    candidates.into_iter().find(|path| path.exists())
}

/// The file name of the kernel, for the licence list.
pub const LIBRARY_NAME: &str = "pdfium.dll";

/// The licence the kernel is under.
pub const LICENSE: &str = "BSD-3-Clause (Chromium's PDFium)";

/// Where the kernel came from, for the records.
pub fn origin() -> &'static str {
    "bblanchon.PDFium.Win32 (NuGet), built from Chromium's PDFium"
}

/// The name of the file, without its folder.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| LIBRARY_NAME.to_string())
}
