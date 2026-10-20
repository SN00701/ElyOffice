//! The tab of a document whose engine is not in the window yet.

use std::path::PathBuf;

use ely_gpui_component::buttons::Button;
use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div};

/// A document the window can show but not edit yet.
pub struct Placeholder {
    pub kind: SharedString,
    pub path: Option<PathBuf>,
    pub module: Option<ely_platform::AppId>,
}

impl Placeholder {
    /// The placeholder for a kind and its file.
    pub fn new(
        kind: SharedString,
        path: Option<PathBuf>,
        module: Option<ely_platform::AppId>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            kind,
            path,
            module,
        }
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| self.kind.to_string());
        let module = self.module;
        let path = self.path.clone();
        let handover = module.map(|module| {
            Button::new(
                "mpelyoffice-handover",
                ely_ui::i18n::text(cx, "mpelyoffice.placeholder.handover", &[]),
            )
            .on_click(move |_, _, _| {
                if let Some(path) = &path {
                    if let Err(error) = ely_platform::apps::open(module, &[path.clone()]) {
                        log::error!("office: {} did not open: {error}", module.name());
                    }
                }
            })
        });
        let mut parts = vec![
            div().text_sm().child(self.kind.clone()).into_any_element(),
            div().text_sm().child(name).into_any_element(),
            div()
                .text_sm()
                .child(ely_ui::i18n::text(cx, "mpelyoffice.placeholder.engine", &[]))
                .into_any_element(),
        ];
        if let Some(button) = handover {
            parts.push(button.into_any_element());
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().flex().flex_col().gap_2().children(parts))
            .into_any_element()
    }
}
