//! The LibreOfficeKit engine: the document kinds LibreOffice reads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::bail;
use ely_lok::{Doc, LokView, Worker};
use gpui::{App, AppContext};

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
    doc: Doc,
    path: PathBuf,
    /// The format it is written back in: the extension it came with.
    format: String,
    /// The history mark of the last write.
    mark: String,
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
        // The kernel loads, measures and reports its history.
        let (doc, laid, mark) = {
            let mut worker = self.worker.lock().expect("the worker lock");
            let doc = worker.unlock(&url, "")?;
            let laid = ely_lok::layout(&mut worker, &doc)?;
            let mark = worker.values(&doc, ".uno:Undo")?;
            (doc, laid, mark)
        };
        // The view makes itself; it is an entity already.
        let view = LokView::open(Arc::clone(&self.worker), doc.clone(), laid, cx);
        let id = self.next;
        self.next += 1;
        self.documents.insert(
            id,
            LokDoc {
                doc,
                path: path.clone(),
                format,
                mark,
            },
        );
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        Ok(Opened {
            id,
            view: view.into(),
            name: name.into(),
        })
    }

    fn commands(&self, _id: DocId) -> Vec<CommandSpec> {
        // The commands of a document come from the kernel itself; the shell
        // asks for them when it builds its menus.
        Vec::new()
    }

    fn text(&self, id: DocId, _cx: &App) -> Option<String> {
        let document = self.documents.get(&id)?;
        // The text of a document is its own; the shell asks the kernel.
        let mut worker = self.worker.lock().ok()?;
        worker.values(&document.doc, ".uno:Modified").ok()
    }

    fn modified(&self, id: DocId, _cx: &App) -> bool {
        let Some(document) = self.documents.get(&id) else {
            return false;
        };
        let Ok(mut worker) = self.worker.lock() else {
            return false;
        };
        // The kernel's history mark moves when the document changes.
        match worker.values(&document.doc, ".uno:Undo") {
            Ok(mark) => mark != document.mark,
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

    fn write(&mut self, id: DocId, path: &Path, _cx: &App) -> anyhow::Result<()> {
        let Some(document) = self.documents.get(&id) else {
            bail!("the document is not open");
        };
        let format = document.format.clone();
        let url = ely_lok::file_url(path);
        let mut worker = self.worker.lock().expect("the worker lock");
        document.doc.save_owned(&mut worker, &url, &format)?;
        if let Some(document) = self.documents.get_mut(&id) {
            document.path = path.to_path_buf();
            document.mark = worker.values(&document.doc, ".uno:Undo")?;
        }
        Ok(())
    }

    fn encoding(&self, _id: DocId) -> Option<crate::Encoding> {
        None
    }

    fn close(&mut self, id: DocId) {
        if let Some(document) = self.documents.remove(&id) {
            if let Ok(mut worker) = self.worker.lock() {
                let _ = worker.close_doc(document.doc);
            }
        }
    }
}
