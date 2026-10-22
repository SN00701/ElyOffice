//! The writing of the documents: save, save as, and the list of the files
//! that were opened.

use std::path::{Path, PathBuf};

use ely_engine::EngineKind;
use gpui::Context;

use super::Office;
use crate::kinds;

/// The path a document is written to: the one that was chosen, with the
/// extension of its kind when it has none.
pub fn with_extension(path: &Path, kind: EngineKind) -> PathBuf {
    if path.extension().is_some() {
        return path.to_path_buf();
    }
    let mut path = path.to_path_buf();
    path.set_extension(kind.default_extension);
    path
}

/// The name a new document is offered under: its own name, with the
/// extension of its kind when it has none.
pub fn suggested_name(name: &str, kind: EngineKind) -> String {
    if Path::new(name).extension().is_some() {
        return name.to_string();
    }
    format!("{name}.{}", kind.default_extension)
}

impl Office {
    pub(super) fn save_all(&mut self, cx: &mut Context<Self>) -> usize {
        let mut written = 0;
        for index in 0..self.tabs.len() {
            if self.save_at(index, cx) {
                written += 1;
            }
        }
        written
    }

    /// Writes the active document back to its file.
    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.active else {
            return;
        };
        self.save_at(index, cx);
    }

    /// Asks for another name, then writes the active document there.
    pub(super) fn save_as(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.active else {
            return;
        };
        let Some(tab) = self.tabs.get(index).map(|tab| (tab.kind, tab.name.to_string())) else {
            return;
        };
        let (kind, name) = tab;
        // A new document has no name of its own yet; it gets the extension of
        // its kind, so the file says what it is.
        let suggested = suggested_name(&name, kind);
        let label = kinds::label(kind, cx).to_string();
        cx.spawn(
            async move |this, cx| {
                let picked = cx
                    .background_executor()
                    .spawn(async move {
                        rfd::FileDialog::new()
                            .set_file_name(&suggested)
                            .add_filter(
                                &label,
                                &kind.extensions.iter().copied().collect::<Vec<_>>(),
                            )
                            .save_file()
                    })
                    .await;
                let Some(path) = picked else {
                    return;
                };
                let _ = this.update(cx, |office, cx| office.save_as_to(index, path, cx));
            },
        )
        .detach();
    }

    /// Writes one document to another file, and points the tab at it.
    pub(super) fn save_as_to(&mut self, index: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        // A name without an extension gets the one of the kind, so the file
        // says what it is.
        let kind = tab.kind;
        let path = with_extension(&path, kind);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.name = name.into();
            tab.path = Some(path.clone());
        }
        self.save_at(index, cx);
    }

    /// Writes one document back; false when it was not written.
    pub(super) fn save_at(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.tabs.get(index) else {
            return false;
        };
        let (kind, doc, path) = (tab.kind, tab.doc, tab.path.clone());
        let Some(doc) = doc else {
            return false;
        };
        let Some(path) = path else {
            self.notice = Some(ely_ui::i18n::text(cx, "mpelyoffice.notice.no_name", &[]));
            cx.notify();
            return false;
        };
        let Some(engine) = self.engines.get_mut(kind.id) else {
            return false;
        };
        match engine.write(doc, &path, cx) {
            Ok(()) => {
                self.remember(&path);
                self.notice = Some(ely_ui::i18n::text(
                    cx,
                    "mpelyoffice.notice.saved",
                    &[("name", &path.display().to_string())],
                ));
                cx.notify();
                true
            }
            Err(error) => {
                log::error!("office: {} was not saved: {error}", path.display());
                let reason = error.to_string();
                let message = if reason.contains("changed on disk") {
                    ely_ui::i18n::text(
                        cx,
                        "mpelyoffice.notice.external",
                        &[("name", &path.display().to_string())],
                    )
                } else {
                    ely_ui::i18n::text(cx, "mpelyoffice.notice.failed", &[("error", &reason)])
                };
                self.notice = Some(message);
                cx.notify();
                false
            }
        }
    }

    /// Remembers a file for the start page.
    pub(super) fn remember(&mut self, path: &std::path::Path) {
        self.recent.retain(|known| known != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(10);
        self.write_recent();
    }

    /// Writes the list of the files of late, so the start page has it.
    fn write_recent(&self) {
        if self.data.as_os_str().is_empty() {
            return;
        }
        let file = self.data.join("recent.json");
        if let Ok(bytes) = serde_json::to_vec(&self.recent) {
            let _ = std::fs::write(file, bytes);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{suggested_name, with_extension};
    use ely_engine::EngineKind;

    fn document() -> EngineKind {
        EngineKind {
            id: "document",
            extensions: &["docx", "odt", "doc", "rtf"],
            default_extension: "docx",
        }
    }

    #[test]
    fn a_new_document_is_offered_with_the_extension_of_its_kind() {
        assert_eq!(suggested_name("文档 1", document()), "文档 1.docx");
        assert_eq!(suggested_name("报告.docx", document()), "报告.docx");
    }

    #[test]
    fn a_file_without_an_extension_gets_the_one_of_its_kind() {
        let written = with_extension(std::path::Path::new("F:/out/报告"), document());
        assert_eq!(written.file_name().unwrap(), "报告.docx");
        let kept = with_extension(std::path::Path::new("F:/out/报告.rtf"), document());
        assert_eq!(kept.file_name().unwrap(), "报告.rtf");
    }
}