//! How the shell shows each kind of document.

use ely_engine::EngineKind;
use ely_gpui_component::primitives::IconName;
use gpui::SharedString;

/// The name of a kind, in the user's language.
pub fn label(kind: EngineKind, cx: &gpui::App) -> SharedString {
    let key = match kind.id {
        "text" => "mpelyoffice.kind.text",
        "markdown" => "mpelyoffice.kind.markdown",
        "document" => "mpelyoffice.kind.document",
        "spreadsheet" => "mpelyoffice.kind.spreadsheet",
        "presentation" => "mpelyoffice.kind.presentation",
        "pdf" => "mpelyoffice.kind.pdf",
        _ => "mpelyoffice.kind.unknown",
    };
    ely_ui::i18n::text(cx, key, &[])
}

/// The mark that tells the kinds apart in the tab strip.
pub fn mark(kind: EngineKind) -> IconName {
    match kind.id {
        "text" => IconName::FileText,
        "markdown" => IconName::NotebookPen,
        "document" => IconName::FileType,
        "spreadsheet" => IconName::FileSpreadsheet,
        "presentation" => IconName::BookOpen,
        _ => IconName::File,
    }
}

/// The application that holds this kind's engine.
pub fn module(kind: EngineKind) -> Option<ely_platform::AppId> {
    match kind.id {
        "document" => Some(ely_platform::AppId::Doc),
        "spreadsheet" => Some(ely_platform::AppId::Table),
        "presentation" => Some(ely_platform::AppId::Show),
        "pdf" => Some(ely_platform::AppId::Pdf),
        "markdown" => Some(ely_platform::AppId::Mark),
        _ => None,
    }
}
