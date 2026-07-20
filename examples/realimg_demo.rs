//! Render a page with a REAL jpeg embedded, for a visual check.
use mupdf::pdf::{PageImageSource, PdfDocument};
use mupdf::{Point, Rect, Size};
fn main() {
    let jpeg = include_bytes!("../tests/assets/real.jpg");
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::A4).unwrap();
    page.insert_image(
        &mut doc,
        Rect { x0: 72.0, y0: 400.0, x1: 232.0, y1: 560.0 },
        PageImageSource::Bytes { data: jpeg, format_hint: Some("jpeg") },
        Default::default(),
    ).unwrap();
    let mut shape = mupdf::shape::Shape::new(&mut page).unwrap();
    shape.insert_text(Point::new(72.0, 600.0), "Real photo embedded (picsum 160x160 jpeg)", &mupdf::shape::TextOptions::default()).unwrap().commit(&mut doc, true).unwrap();
    doc.save("/tmp/realimg.pdf").unwrap();
    let v = mupdf_mini_rs::ViewerDocument::open("/tmp/realimg.pdf").unwrap();
    v.save_page_png(0, 2.0, 0, "/tmp/realimg.png").unwrap();
    println!("rendered /tmp/realimg.png");
}
