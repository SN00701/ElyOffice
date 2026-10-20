//! The contract every engine answers to, run on the engines there are.

use gpui::TestAppContext;

use crate::text::TextEngine;
use crate::EngineDyn;

#[gpui::test]
fn the_text_engine_answers_the_contract(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let mut engine = TextEngine::text();
        crate::contract::check(&mut engine, cx).expect("the text engine answers the contract");
    });
}

#[gpui::test]
fn the_markdown_engine_answers_the_contract(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let mut engine = TextEngine::markdown();
        crate::contract::check(&mut engine, cx).expect("the markdown engine answers the contract");
    });
}

#[gpui::test]
fn two_engines_keep_their_documents_apart(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let mut text = TextEngine::text();
        let mut markdown = TextEngine::markdown();
        let first = text
            .open(crate::Source::Blank, cx)
            .expect("the text engine opens");
        let second = markdown
            .open(crate::Source::Blank, cx)
            .expect("the markdown engine opens");
        // A document's number is its own engine's, so the engines are
        // compared by what they do, not by their numbers.
        assert!(text.text(first.id, cx).is_some());
        assert!(markdown.text(second.id, cx).is_some());
        // Closing a document in one engine leaves the other alone.
        text.close(first.id);
        assert!(text.text(first.id, cx).is_none());
        assert!(markdown.text(second.id, cx).is_some());
    });
}
