//! The rules the shell follows for each kind of document.

use gpui::TestAppContext;

use ely_engine::{EngineKind, Registry};

use crate::kinds;

fn kind(id: &str) -> EngineKind {
    Registry::new()
        .kind(id)
        .unwrap_or_else(|| panic!("the {id} engine is there"))
}

#[gpui::test]
fn every_kind_has_a_label_and_a_mark(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        ely_ui::i18n::install_as("zh-CN", cx);
        let registry = Registry::new();
        for kind in registry.kinds() {
            let label = kinds::label(*kind, cx);
            assert!(!label.is_empty(), "{} has no label", kind.id);
            // The label differs from the plain name, so the kinds are told apart.
            assert_ne!(label.to_string(), kind.id, "{} shows its raw name", kind.id);
        }
    });
}

#[gpui::test]
fn the_kinds_tell_each_other_apart(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let registry = Registry::new();
        let mut marks = Vec::new();
        for kind in registry.kinds() {
            let mark = kinds::mark(*kind);
            assert!(
                !marks.contains(&mark),
                "{} shares its mark with another kind",
                kind.id
            );
            marks.push(mark);
        }
    });
}

#[gpui::test]
fn the_kinds_point_at_the_module_apps_that_hold_their_engines(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        assert_eq!(kinds::module(kind("document")), Some(ely_platform::AppId::Doc));
        assert_eq!(
            kinds::module(kind("spreadsheet")),
            Some(ely_platform::AppId::Table)
        );
        assert_eq!(
            kinds::module(kind("presentation")),
            Some(ely_platform::AppId::Show)
        );
        assert_eq!(kinds::module(kind("pdf")), Some(ely_platform::AppId::Pdf));
        assert_eq!(kinds::module(kind("markdown")), Some(ely_platform::AppId::Mark));
        assert_eq!(kinds::module(kind("text")), None);
    });
}
