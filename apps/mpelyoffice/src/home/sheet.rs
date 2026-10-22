//! The sheet that asks what kind of document to make.

use ely_gpui_component::buttons::Button;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use super::{Home, HomeEvent};
use crate::kinds;
use ely_engine::EngineKind;

/// The kinds of document that can be made.
fn kinds_of() -> [(&'static str, EngineKind); 6] {
    [
        (
            "mpelyoffice.home.new_document",
            EngineKind {
                id: "document",
                extensions: &["docx", "odt", "doc", "rtf"],
                default_extension: "docx",
            },
        ),
        (
            "mpelyoffice.home.new_spreadsheet",
            EngineKind {
                id: "spreadsheet",
                extensions: &["xlsx", "ods", "xls", "csv", "tsv"],
                default_extension: "xlsx",
            },
        ),
        (
            "mpelyoffice.home.new_presentation",
            EngineKind {
                id: "presentation",
                extensions: &["pptx", "odp", "ppt"],
                default_extension: "pptx",
            },
        ),
        (
            "mpelyoffice.home.new_pdf",
            EngineKind {
                id: "pdf",
                extensions: &["pdf"],
                default_extension: "pdf",
            },
        ),
        (
            "mpelyoffice.home.new_markdown",
            EngineKind {
                id: "markdown",
                extensions: &["md", "markdown"],
                default_extension: "md",
            },
        ),
        (
            "mpelyoffice.home.new_text",
            EngineKind {
                id: "text",
                extensions: &["txt", "log", "ini", "conf"],
                default_extension: "txt",
            },
        ),
    ]
}

impl Home {
    /// The sheet that asks what kind of document to make.
    pub(super) fn new_sheet(
        &self,
        cx: &Context<Self>,
        theme: &ely_gpui_component::theme::Theme,
    ) -> impl IntoElement {
        let this = cx.entity();
        let cancel_this = this.clone();
        let mut grid = div().flex().flex_wrap().gap_3();
        for (key, kind) in kinds_of() {
            let this = this.clone();
            let label = ely_ui::i18n::text(cx, key, &[]);
            grid = grid.child(
                Button::new(format!("home-new-{}", kind.id), label)
                    .icon(kinds::mark(kind))
                    .on_click(move |_, _, cx| {
                        let _ = this.update(cx, |home, cx| {
                            home.newing = false;
                            cx.emit(HomeEvent::New(kind));
                        });
                    }),
            );
        }
        let title = ely_ui::i18n::text(cx, "mpelyoffice.home.new_title", &[]);
        let cancel = ely_ui::i18n::text(cx, "mpelyoffice.home.cancel", &[]);
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.colors.bg.opacity(0.55))
            .child(
                div()
                    .w(px(560.0))
                    .rounded_lg()
                    .bg(theme.colors.surface)
                    .border_1()
                    .shadow_lg()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_lg().child(title))
                    .child(grid)
                    .child(Button::new("home-new-cancel", cancel).on_click(move |_, _, cx| {
                        let _ = cancel_this.update(cx, |home, _| home.newing = false);
                    })),
            )
    }
}
