//! The engines the one window drives, and the rules they answer to.

pub mod contract;
#[cfg(test)]
mod contract_tests;
pub mod encoding;
pub mod engine;
pub mod lok;
#[cfg(test)]
mod lok_tests;
pub mod pdf;
#[cfg(test)]
mod pdf_tests;
pub mod pdfium;
pub mod registry;
pub mod text;

use std::path::{Path, PathBuf};

use gpui::{App, AnyView};

pub use encoding::{Encoding, Fingerprint};
pub use engine::{CommandSpec, DocId, EngineKind, Opened, Source};
pub use lok::LokEngine;
pub use pdf::PdfEngine;
pub use registry::Registry;
pub use text::TextEngine;

/// The folder the engines put what they draw in.
pub fn work_dir(name: &str) -> PathBuf {
    let base = ely_platform::data_dir(ely_platform::AppId::Office)
        .unwrap_or_else(|_| std::env::temp_dir().join("MPelyoffice"));
    base.join("work").join(name)
}

/// What one engine can do with a document.
///
/// The trait is object safe on purpose: the one window holds documents of
/// several kinds at once, so the shell talks to engines through this trait
/// rather than through a generic parameter. A new kind of document is a new
/// implementation of this trait, not a new application.
///
/// An engine owns what it knows about its documents — the file, the encoding,
/// the fingerprint and the revision that was saved. The view it hands back
/// owns the text being edited and counts its revisions. That split keeps the
/// trait free of `&mut App` and lets the shell hold engines of several kinds
/// side by side.
pub trait EngineDyn: 'static {
    /// What this engine is for.
    fn kind(&self) -> EngineKind;

    /// Opens a document and hands back its view.
    fn open(&mut self, source: Source, cx: &mut App) -> anyhow::Result<Opened>;

    /// The commands this engine offers for a document.
    fn commands(&self, id: DocId) -> Vec<CommandSpec>;

    /// The document's text, for the shell to save or show.
    fn text(&self, id: DocId, cx: &App) -> Option<String>;

    /// Whether the document has changes that are not saved.
    fn modified(&self, id: DocId, cx: &App) -> bool;

    /// The file the document lives in, if it has one.
    fn path(&self, id: DocId) -> Option<PathBuf>;

    /// Points the document at another file, as "save as" does.
    fn rename(&mut self, id: DocId, path: PathBuf);

    /// Writes the document to a file.
    fn write(&mut self, id: DocId, path: &Path, cx: &App) -> anyhow::Result<()>;

    /// The encoding the file is read and written in.
    fn encoding(&self, id: DocId) -> Option<Encoding>;

    /// Closes the document and lets the engine drop it.
    fn close(&mut self, id: DocId);

    /// Whether the document is still being loaded.
    ///
    /// An engine that takes long to open a document says so here; the
    /// window shows a placeholder until the document is there.
    fn is_loading(&self, _id: DocId) -> bool {
        false
    }
}

/// The view of a document, as the shell puts it in a tab.
pub type View = AnyView;
