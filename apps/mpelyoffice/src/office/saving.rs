//! The writing of the documents: save, save as, and the list of the files
//! that were opened.

use std::path::PathBuf;

use gpui::Context;

use super::Office;
use crate::kinds;

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
        let label = kinds::label(kind, cx).to_string();
        cx.spawn(
            async move |this, cx| {
                let picked = cx
                    .background_executor()
                    .spawn(async move {
                        rfd::FileDialog::new()
                            .set_file_name(&name)
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
        if self.tabs.get(index).is_none() {
            return;
        }
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
    }
}
