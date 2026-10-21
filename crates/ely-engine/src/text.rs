//! The text engine: a plain editor over a text file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ely_gpui_component::editor::{CodeEditor, EditorEvent};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
};

use crate::encoding::{self, Encoding, Fingerprint};
use crate::engine::{CommandSpec, DocId, EngineKind, Opened, Source};
use crate::EngineDyn;

/// The engine for text and Markdown files.
pub struct TextEngine {
    kind: EngineKind,
    documents: HashMap<DocId, Document>,
    next: DocId,
}

/// One open document.
struct Document {
    path: Option<PathBuf>,
    view: Entity<TextView>,
    /// The revision that was last written; the view counts its own.
    saved: u64,
    /// What the file looked like when it was read or written.
    fingerprint: Fingerprint,
    /// How the file is read and written.
    encoding: Encoding,
}

impl TextEngine {
    /// The engine for the given kind.
    pub fn new(kind: EngineKind) -> Self {
        Self {
            kind,
            documents: HashMap::new(),
            next: 0,
        }
    }

    /// The text engine for plain text.
    pub fn text() -> Self {
        Self::new(EngineKind {
            id: "text",
            extensions: &["txt", "log", "ini", "conf"],
            default_extension: "txt",
        })
    }

    /// The text engine for Markdown.
    pub fn markdown() -> Self {
        Self::new(EngineKind {
            id: "markdown",
            extensions: &["md", "markdown"],
            default_extension: "md",
        })
    }
}

impl EngineDyn for TextEngine {
    fn kind(&self) -> EngineKind {
        self.kind
    }

    fn open(&mut self, source: Source, cx: &mut App) -> anyhow::Result<Opened> {
        let (path, loaded) = match source {
            Source::File(path) => {
                let loaded = encoding::load(&path);
                (Some(path), loaded)
            }
            Source::Blank => (
                None,
                encoding::Loaded {
                    text: String::new(),
                    encoding: Encoding::Utf8,
                    fingerprint: Fingerprint {
                        len: 0,
                        modified: None,
                    },
                    truncated: false,
                },
            ),
        };
        let name = path
            .as_ref()
            .map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
            })
            .unwrap_or_else(|| self.kind.default_extension.to_string());
        let view = cx.new(|cx| TextView::new(loaded.text.clone(), cx));
        let id = self.next;
        self.next += 1;
        self.documents.insert(
            id,
            Document {
                path,
                view: view.clone(),
                saved: view.read(cx).revision(),
                fingerprint: loaded.fingerprint,
                encoding: loaded.encoding,
            },
        );
        Ok(Opened {
            id,
            view: view.into(),
            name: name.into(),
        })
    }

    fn commands(&self, _id: DocId) -> Vec<CommandSpec> {
        Vec::new()
    }

    fn text(&self, id: DocId, cx: &App) -> Option<String> {
        self.documents
            .get(&id)
            .and_then(|doc| doc.view.read(cx).text(cx))
    }

    fn modified(&self, id: DocId, cx: &App) -> bool {
        self.documents
            .get(&id)
            .map(|doc| doc.view.read(cx).revision() != doc.saved)
            .unwrap_or(false)
    }

    fn path(&self, id: DocId) -> Option<PathBuf> {
        self.documents.get(&id).and_then(|doc| doc.path.clone())
    }

    fn rename(&mut self, id: DocId, path: PathBuf) {
        if let Some(doc) = self.documents.get_mut(&id) {
            doc.path = Some(path);
        }
    }

    fn write(&mut self, id: DocId, path: &Path, cx: &mut App) -> anyhow::Result<()> {
        let Some(doc) = self.documents.get_mut(&id) else {
            anyhow::bail!("the document is not open");
        };
        let content = doc.view.read(cx).text(cx).unwrap_or_default();
        // An outside change is not overwritten.
        if let Some(current) = encoding::fingerprint(path) {
            if current != doc.fingerprint {
                anyhow::bail!("the file changed on disk");
            }
        }
        encoding::store(path, &content, doc.encoding)?;
        doc.fingerprint = encoding::fingerprint(path).unwrap_or(doc.fingerprint);
        doc.saved = doc.view.read(cx).revision();
        Ok(())
    }

    fn encoding(&self, id: DocId) -> Option<Encoding> {
        self.documents.get(&id).map(|doc| doc.encoding)
    }

    fn close(&mut self, id: DocId, _cx: &App) {
        self.documents.remove(&id);
    }
}

/// The document a tab shows.
pub struct TextView {
    text: String,
    /// Counts the changes; the engine remembers what was last saved.
    revision: u64,
    editor: Option<Entity<CodeEditor>>,
    subscription: Option<gpui::Subscription>,
}

impl TextView {
    /// A document with the file's text.
    pub fn new(text: String, _cx: &mut Context<Self>) -> Self {
        Self {
            text,
            revision: 0,
            editor: None,
            subscription: None,
        }
    }

    /// The text the editor holds.
    pub fn text(&self, cx: &App) -> Option<String> {
        match &self.editor {
            Some(editor) => Some(editor.read(cx).text().to_string()),
            None => Some(self.text.clone()),
        }
    }

    /// How many times the document has changed.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The editor, made on the first frame that needs it.
    fn editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<CodeEditor>> {
        if let Some(editor) = &self.editor {
            return Some(editor.clone());
        }
        let text = self.text.clone();
        let editor = cx.new(|cx| CodeEditor::new(text, window, cx));
        let subscription = cx.subscribe(
            &editor,
            |view: &mut TextView, _, event: &EditorEvent, cx: &mut Context<Self>| {
                if matches!(event, EditorEvent::Changed) {
                    view.revision += 1;
                    cx.notify();
                }
            },
        );
        self.subscription = Some(subscription);
        self.editor = Some(editor.clone());
        Some(editor)
    }
}

impl Render for TextView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = self.editor(window, cx) {
            return div().size_full().child(editor).into_any_element();
        }
        div().size_full().into_any_element()
    }
}
