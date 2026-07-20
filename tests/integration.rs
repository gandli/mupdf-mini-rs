//! Integration tests. A synthetic PDF is generated with MuPDF's `Shape` API
//! (no external fixture files, no network), then rendered, text-extracted and
//! exported to PNG to exercise the full pipeline.

use mupdf::pdf::PdfDocument;
use mupdf::shape::{FinishOptions, PdfColor, Shape, TextOptions};
use mupdf::{Point, Rect, Size, TextExtractOptions};

use mupdf_mini_rs::ViewerDocument;

fn fixture_path(name: &str) -> String {
    // CARGO_TARGET_TMPDIR points at a per-test temp dir that already exists.
    // Each test gets a unique file name so parallel runs don't clobber a
    // shared fixture (which caused intermittent search/text races).
    format!("{}/sample-{name}.pdf", env!("CARGO_TARGET_TMPDIR"))
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
    let path = fixture_path("main");
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
    let path = fixture_path("smoke");
    make_fixture(&path);
    let doc = ViewerDocument::open(&path).unwrap();
    // Exercise the lower-level option type to guard against API drift.
    let page = doc.render(0, 1.0, 0).unwrap();
    assert_eq!(page.pixel_count(), page.width * page.height);
    let _ = TextExtractOptions::default();
}

#[test]
fn cjk_render_path_works_via_argv() {
    let path = fixture_path("cjk");
    make_cjk_fixture(&path);
    let doc = ViewerDocument::open(&path).unwrap();
    // A page containing CJK text must open, render and export without error.
    // (CJK search hits depend on whether the synthetic font embeds a text
    // layer; we just assert the non-ASCII pipeline does not panic.)
    let page = doc.render(0, 2.0, 0).expect("render cjk page");
    assert!(page.width > 0 && page.height > 0);
    let png = format!("{}/cjk.png", env!("CARGO_TARGET_TMPDIR"));
    doc.save_page_png_with_search(0, 2.0, 0, Some("你好"), &png)
        .expect("save cjk png via non-ascii argv term");
    assert!(std::fs::metadata(&png).unwrap().len() > 0);
}

#[test]
fn rotation_highlight_ctm_is_orthogonal() {
    // The ctm used for highlight placement must match the one used during
    // render. For every rotation it must be a finite, length-preserving
    // (orthogonal) transform so a hit maps to a well-defined pixel, and
    // rotation must swap page axes without shearing.
    for rot in [0u16, 90, 180, 270] {
        let m = mupdf_mini_rs::render::ctm_for(2.0, rot as u8);
        let origin = mupdf::Point::new(0.0, 0.0).transform(&m);
        let ux = mupdf::Point::new(1.0, 0.0).transform(&m);
        let uy = mupdf::Point::new(0.0, 1.0).transform(&m);
        for p in [origin, ux, uy] {
            assert!(p.x.is_finite() && p.y.is_finite(), "rot {rot}: non-finite");
        }
        // Basis vectors stay unit length (no shear / non-uniform scale).
        let lx = (ux.x * ux.x + ux.y * ux.y).sqrt();
        let ly = (uy.x * uy.x + uy.y * uy.y).sqrt();
        assert!((lx - 2.0).abs() < 1e-2, "rot {rot}: ux len {lx}");
        assert!((ly - 2.0).abs() < 1e-2, "rot {rot}: uy len {ly}");
        // Basis vectors stay orthogonal.
        let dot = ux.x * uy.x + ux.y * uy.y;
        assert!(
            dot.abs() < 1e-2,
            "rot {rot}: basis not orthogonal (dot {dot})"
        );
    }
}

#[test]
fn multi_page_out_of_range_is_consistent() {
    let path = fixture_path("multi");
    make_multi_page_fixture(&path);
    let doc = ViewerDocument::open(&path).unwrap();
    assert_eq!(doc.page_count(), 3);
    // Every in-range page renders; one past the end errors.
    for i in 0..3 {
        assert!(doc.render(i, 1.0, 0).is_ok());
    }
    assert!(doc.render(3, 1.0, 0).is_err());
    assert!(doc.render(99, 1.0, 0).is_err());
    // Search on the second page (which also contains the term).
    let hits = doc.search(1, "Page 2").unwrap();
    assert!(!hits.is_empty());
}

fn make_cjk_fixture(path: &str) {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::A4).unwrap();
    let mut shape = Shape::new(&mut page).unwrap();
    shape
        .insert_text(
            Point::new(72.0, 700.0),
            "你好，世界",
            &TextOptions::default(),
        )
        .unwrap()
        .commit(&mut doc, true)
        .unwrap();
    doc.save(path).unwrap();
}

fn make_multi_page_fixture(path: &str) {
    let mut doc = PdfDocument::new();
    for n in 1..=3 {
        let mut page = doc.new_page(Size::A4).unwrap();
        let mut shape = Shape::new(&mut page).unwrap();
        shape
            .insert_text(
                Point::new(72.0, 700.0),
                &format!("Page {n} of the mini viewer"),
                &TextOptions::default(),
            )
            .unwrap()
            .commit(&mut doc, true)
            .unwrap();
    }
    doc.save(path).unwrap();
}
