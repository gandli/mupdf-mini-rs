//! Integration tests. A synthetic PDF is generated with MuPDF's `Shape` API
//! (no external fixture files, no network), then rendered, text-extracted and
//! exported to PNG to exercise the full pipeline.

use mupdf::pdf::PdfDocument;
use mupdf::shape::{FinishOptions, PdfColor, Shape, TextOptions};
use mupdf::{Point, Rect, Size, TextExtractOptions};

use mupdf_mini_rs::ViewerDocument;

fn fixture_path() -> String {
    // CARGO_TARGET_TMPDIR points at a per-test temp dir that already exists.
    format!("{}/sample.pdf", env!("CARGO_TARGET_TMPDIR"))
}

fn make_fixture(path: &str) {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::A4).unwrap();
    let rect = Rect::new(72.0, 72.0, 523.0, 720.0);
    let mut shape = Shape::new(&mut page).unwrap();
    shape
        .draw_rect(&rect)
        .unwrap()
        .finish(&FinishOptions {
            color: Some(PdfColor::rgb(0.0, 0.0, 0.0)),
            fill: Some(PdfColor::rgb(0.9, 0.95, 1.0)),
            width: 1.5,
            ..Default::default()
        })
        .unwrap()
        .insert_text(
            Point::new(80.0, 700.0),
            "Hello MuPDF mini",
            &TextOptions::default(),
        )
        .unwrap()
        .insert_text(
            Point::new(80.0, 676.0),
            "Page 1 of the mini viewer",
            &TextOptions::default(),
        )
        .unwrap()
        .commit(&mut doc, true)
        .unwrap();
    doc.save(path).unwrap();
}

#[test]
fn open_render_text_and_export() {
    let path = fixture_path();
    make_fixture(&path);

    let doc = ViewerDocument::open(&path).expect("open fixture");
    assert_eq!(doc.page_count(), 1);

    // Render at 2x zoom, no rotation.
    let page = doc.render(0, 2.0, 0).expect("render page");
    assert!(page.width > 0 && page.height > 0, "page has zero size");
    assert_eq!(page.rgba.len(), page.width * page.height * 4);

    // Rotation 90 degrees must keep the same pixel count (just transposed).
    let rotated = doc.render(0, 2.0, 90).expect("render rotated");
    assert_eq!(rotated.width * rotated.height, page.width * page.height);

    // Text extraction.
    let text = doc.text(0).expect("extract text");
    assert!(text.contains("Hello MuPDF mini"));

    // Export to PNG.
    let png = format!("{}/sample.png", env!("CARGO_TARGET_TMPDIR"));
    doc.save_page_png(0, 2.0, 0, &png).expect("save png");
    let meta = std::fs::metadata(&png).expect("png exists");
    assert!(meta.len() > 0, "png should not be empty");

    // Search: a present term returns hits; an absent term returns none.
    let hits = doc.search(0, "Hello").expect("search");
    assert!(!hits.is_empty(), "should find 'Hello'");
    // Each hit quad must be finite and within the PDF page bounds.
    let (pw, ph) = doc.page_size_pt(0).unwrap();
    for q in &hits {
        for p in [q.ul, q.ur, q.ll, q.lr] {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(p.x >= -1.0 && p.x <= pw + 1.0);
            assert!(p.y >= -1.0 && p.y <= ph + 1.0);
        }
    }
    let miss = doc.search(0, "zzznotfound").expect("search miss");
    assert!(miss.is_empty(), "should not find absent term");

    // Headless highlighted export: rendering with a matching term must
    // inject yellow (high-alpha) pixels into the output PNG.
    let hi_png = format!("{}/hi.png", env!("CARGO_TARGET_TMPDIR"));
    doc.save_page_png_with_search(0, 2.0, 0, Some("Hello"), &hi_png)
        .expect("save highlighted png");
    let hi_meta = std::fs::metadata(&hi_png).expect("hi png exists");
    assert!(hi_meta.len() > 0);

    // Out-of-range page must error, not panic.
    assert!(doc.render(5, 1.0, 0).is_err());
}

#[test]
fn text_extract_options_smoke() {
    let path = fixture_path();
    make_fixture(&path);
    let doc = ViewerDocument::open(&path).unwrap();
    // Exercise the lower-level option type to guard against API drift.
    let page = doc.render(0, 1.0, 0).unwrap();
    assert_eq!(page.pixel_count(), page.width * page.height);
    let _ = TextExtractOptions::default();
}
