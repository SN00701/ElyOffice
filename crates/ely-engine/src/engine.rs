//! The engine a document needs, and the commands it offers.

use gpui::SharedString;

/// What one engine is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EngineKind {
    /// The name of the engine, as the shell shows it.
    pub id: &'static str,
    /// The extensions it answers to.
    pub extensions: &'static [&'static str],
    /// The extension a new document of this engine gets.
    pub default_extension: &'static str,
}

impl EngineKind {
    /// Whether the extension is one of this engine's.
    pub fn answers(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }
}

/// One command an engine offers; the shell shows it in the menu and the ribbon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// The command's name, as the shell knows it.
    pub id: &'static str,
    /// The message key for its label.
    pub label: &'static str,
    /// The key of its chord, if it has one.
    pub keys: Option<&'static str>,
    /// Whether it can run now; false with a reason.
    pub enabled: bool,
    /// Why it cannot run, when it cannot.
    pub reason: Option<&'static str>,
}

impl CommandSpec {
    /// A command that can run.
    pub fn ready(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            keys: None,
            enabled: true,
            reason: None,
        }
    }

    /// A command that cannot run, and why.
    pub fn blocked(id: &'static str, label: &'static str, reason: &'static str) -> Self {
        Self {
            id,
            label,
            keys: None,
            enabled: false,
            reason: Some(reason),
        }
    }
}

/// A document's identity inside its engine.
pub type DocId = u64;

/// Where a document came from or goes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A file on disk.
    File(std::path::PathBuf),
    /// A document that has no file yet.
    Blank,
}

/// What an engine hands back when it opens a document.
pub struct Opened {
    /// The document's identity inside the engine.
    pub id: DocId,
    /// The view to put in the tab.
    pub view: gpui::AnyView,
    /// The name to show on the tab.
    pub name: SharedString,
}
