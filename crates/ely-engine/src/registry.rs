//! Which engine answers for a file.

use std::collections::HashMap;
use std::path::Path;

use crate::engine::EngineKind;

/// The engines the one window knows, and the file types each answers for.
pub fn kinds() -> Vec<EngineKind> {
    vec![
        EngineKind {
            id: "text",
            extensions: &["txt", "log", "ini", "conf"],
            default_extension: "txt",
        },
        EngineKind {
            id: "markdown",
            extensions: &["md", "markdown"],
            default_extension: "md",
        },
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
        EngineKind {
            id: "pdf",
            extensions: &["pdf"],
            default_extension: "pdf",
        },
    ]
}

/// The engines, and the file types each answers for.
#[derive(Clone, Debug)]
pub struct Registry {
    kinds: Vec<EngineKind>,
    by_extension: HashMap<String, usize>,
}

impl Registry {
    /// The registry of the engines the shell has.
    pub fn new() -> Self {
        let kinds = kinds();
        let mut by_extension = HashMap::new();
        for (index, kind) in kinds.iter().enumerate() {
            for extension in kind.extensions {
                by_extension.insert(extension.to_ascii_lowercase(), index);
            }
        }
        Self { kinds, by_extension }
    }

    /// The kinds the shell knows, in order.
    pub fn kinds(&self) -> &[EngineKind] {
        &self.kinds
    }

    /// The engine that answers for a path.
    pub fn kind_for(&self, path: &Path) -> Option<EngineKind> {
        let extension = path.extension()?.to_str()?;
        self.kind_for_extension(extension)
    }

    /// The engine that answers for an extension.
    pub fn kind_for_extension(&self, extension: &str) -> Option<EngineKind> {
        let index = self.by_extension.get(&extension.to_ascii_lowercase())?;
        Some(self.kinds[*index])
    }

    /// The engine with that name.
    pub fn kind(&self, id: &str) -> Option<EngineKind> {
        self.kinds.iter().copied().find(|kind| kind.id == id)
    }

    /// Whether two engines claim the same extension; that would be a mistake.
    pub fn conflicts(&self) -> Vec<&'static str> {
        let mut seen: HashMap<&'static str, &'static str> = HashMap::new();
        let mut clashes = Vec::new();
        for kind in &self.kinds {
            for extension in kind.extensions {
                match seen.get(extension) {
                    Some(_other) => clashes.push(*extension),
                    None => {
                        seen.insert(extension, kind.id);
                    }
                }
            }
        }
        clashes
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_path_finds_its_engine() {
        let registry = Registry::new();
        let path = |name: &str| PathBuf::from(name);
        assert_eq!(registry.kind_for(&path("a.DOCX")).map(|k| k.id), Some("document"));
        assert_eq!(registry.kind_for(&path("a.xlsx")).map(|k| k.id), Some("spreadsheet"));
        assert_eq!(registry.kind_for(&path("a.pptx")).map(|k| k.id), Some("presentation"));
        assert_eq!(registry.kind_for(&path("a.pdf")).map(|k| k.id), Some("pdf"));
        assert_eq!(registry.kind_for(&path("a.md")).map(|k| k.id), Some("markdown"));
        assert_eq!(registry.kind_for(&path("a.txt")).map(|k| k.id), Some("text"));
        assert_eq!(registry.kind_for(&path("a.unknown")), None);
        assert_eq!(registry.kind_for(&path("noextension")), None);
    }

    #[test]
    fn no_two_engines_claim_the_same_extension() {
        assert_eq!(Registry::new().conflicts(), Vec::<&str>::new());
    }

    #[test]
    fn an_engine_is_found_by_its_name() {
        let registry = Registry::new();
        let kind = registry.kind("text").expect("the text engine is there");
        assert_eq!(kind.default_extension, "txt");
        assert!(kind.answers("TXT"));
        assert!(!kind.answers("md"));
    }
}
