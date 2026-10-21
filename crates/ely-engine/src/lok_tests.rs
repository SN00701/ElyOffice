//! The document engine against a real document.
//!
//! The kernel is needed; when it is not there the test says so and
//! passes, so the suite stays green on a machine without it.

use std::path::PathBuf;

use crate::engine::Source;
use crate::lok::LokEngine;
use crate::EngineDyn;

/// The sample the script prepares.
fn sample() -> Option<PathBuf> {
    let path = std::env::var_os("ELY_TEST_DOCX")?;
    let path = PathBuf::from(path);
    path.exists().then_some(path)
}

/// The kernel, when it is there.
fn kernel() -> Option<PathBuf> {
    match ely_lok::install_dir() {
        Ok(install) => Some(install),
        Err(error) => {
            eprintln!("the kernel is not here, the test is skipped: {error}");
            None
        }
    }
}

#[gpui::test]
fn the_document_engine_opens_a_document(cx: &mut gpui::TestAppContext) {
    let (Some(kernel), Some(sample)) = (kernel(), sample()) else {
        return;
    };
    eprintln!("the kernel is at {}", kernel.display());
    let profile = std::env::temp_dir().join("mpelyoffice-lok-test-profile");
    std::fs::create_dir_all(&profile).expect("the profile folder is there");
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let worker = ely_lok::Worker::launch(&kernel, &profile).expect("the worker starts");
        let kind = LokEngine::kinds()
            .into_iter()
            .find(|kind| kind.id == "document")
            .expect("the document engine is there");
        let mut engine =
            LokEngine::new(kind, std::sync::Arc::new(std::sync::Mutex::new(worker)));
        let opened = engine
            .open(Source::File(sample), cx)
            .expect("the document opens");
        assert!(!opened.name.is_empty(), "the document has a name");
        assert!(
            engine.path(opened.id).is_some(),
            "the document knows its file"
        );
        engine.close(opened.id);
    });
}
