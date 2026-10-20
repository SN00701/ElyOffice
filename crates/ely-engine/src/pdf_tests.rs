//! The PDF engine against a real document.

use std::path::PathBuf;

use crate::engine::Source;
use crate::pdf::PdfEngine;
use crate::EngineDyn;

/// A small PDF with one page and one line of text.
fn sample_pdf() -> Vec<u8> {
    let text = b"BT /F1 28 Tf 72 700 Td (MPelyoffice PDF engine) Tj ET";
    let mut content = format!("<< /Length {} >>\nstream\n", text.len()).into_bytes();
    content.extend_from_slice(text);
    content.extend_from_slice(b"\nendstream");
    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
        content,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let start = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// The kernel, when it is there.
fn kernel() -> Option<crate::pdfium::Pdfium> {
    match crate::pdfium::Pdfium::load() {
        Ok(pdfium) => Some(pdfium),
        Err(error) => {
            eprintln!("the PDFium kernel is not here, the test is skipped: {error}");
            None
        }
    }
}

#[test]
fn the_kernel_reads_a_document() {
    let Some(pdfium) = kernel() else {
        return;
    };
    let path = std::env::temp_dir().join("mpelyoffice-sample.pdf");
    std::fs::write(&path, sample_pdf()).expect("the sample is written");
    let document = pdfium.open(&path).expect("the document opens");
    assert_eq!(pdfium.pages(document), 1);
    let page = pdfium.page(document, 0).expect("the page opens");
    let (width, height) = pdfium.size(page);
    assert!(width > 0.0 && height > 0.0, "the page has a size");
    let text = pdfium.text(page);
    assert!(
        text.contains("MPelyoffice"),
        "the page has its text, got {text:?}"
    );
    pdfium.close_page(page);
    pdfium.close_document(document);
    std::fs::remove_file(&path).ok();
}

#[test]
fn the_kernel_draws_a_page() {
    let Some(pdfium) = kernel() else {
        return;
    };
    let path = std::env::temp_dir().join("mpelyoffice-sample-draw.pdf");
    std::fs::write(&path, sample_pdf()).expect("the sample is written");
    let document = pdfium.open(&path).expect("the document opens");
    let page = pdfium.page(document, 0).expect("the page opens");
    let pixels = pdfium.render(page, 64, 90).expect("the page is drawn");
    assert_eq!(pixels.len(), (64 * 90 * 4) as usize);
    // The page is white, so the text is the only dark thing on it.
    let dark = pixels.chunks_exact(4).filter(|pixel| pixel[0] < 200).count();
    assert!(dark > 0, "the page has something drawn on it");
    pdfium.close_page(page);
    pdfium.close_document(document);
    std::fs::remove_file(&path).ok();
}

#[gpui::test]
fn the_pdf_engine_opens_a_document(cx: &mut gpui::TestAppContext) {
    if kernel().is_none() {
        return;
    }
    let path: PathBuf = std::env::temp_dir().join("mpelyoffice-engine.pdf");
    std::fs::write(&path, sample_pdf()).expect("the sample is written");
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let mut engine = PdfEngine::new();
        let opened = engine
            .open(Source::File(path.clone()), cx)
            .expect("the engine opens the document");
        assert_eq!(engine.pages(opened.id), 1);
        let text = engine.text(opened.id, cx).unwrap_or_default();
        assert!(
            text.contains("MPelyoffice"),
            "the engine reads the text, got {text:?}"
        );
        engine.close(opened.id);
    });
    std::fs::remove_file(&path).ok();
}
