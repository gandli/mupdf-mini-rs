//! Showcase: build a single-page PDF containing text, an embedded raster
//! image, a QR-code-like bitmap, and a drawn table, then render it to PNG.
//! This exercises the document/render pipeline on the kind of mixed-content
//! pages real-world PDFs have.
//!
//! Run: cargo run --example showcase

use mupdf::pdf::{PageImageSource, PdfDocument};
use mupdf::shape::{FinishOptions, Shape, TextOptions};
use mupdf::{Colorspace, Matrix, Pixmap, Point, Rect, Size};

fn make_checker(w: i32, h: i32, c0: [u8; 3], c1: [u8; 3], cells: i32) -> Pixmap {
    let mut pm = Pixmap::new_with_w_h(&Colorspace::device_rgb(), w, h, false).unwrap();
    let s = pm.samples_mut();
    let cw = w / cells;
    let ch = h / cells;
    for y in 0..h {
        let gy = (y / ch.max(1)) % 2;
        for x in 0..w {
            let gx = (x / cw.max(1)) % 2;
            let c = if (gx + gy) % 2 == 0 { c0 } else { c1 };
            let i = ((y * w + x) * 3) as usize;
            s[i] = c[0];
            s[i + 1] = c[1];
            s[i + 2] = c[2];
        }
    }
    pm
}

fn main() {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::A4).unwrap();

    // Title
    let mut shape = Shape::new(&mut page).unwrap();
    shape
        .insert_text(
            Point::new(72.0, 790.0),
            "mupdf-mini-rs showcase: text + image + QR + table",
            &TextOptions::default(),
        )
        .unwrap()
        .commit(&mut doc, true)
        .unwrap();

    // Embedded raster image (top-right): red/blue checker
    let img = make_checker(80, 80, [220, 40, 40], [40, 60, 220], 8);
    let r = Rect {
        x0: 450.0,
        y0: 720.0,
        x1: 530.0,
        y1: 800.0,
    };
    page.insert_image(
        &mut doc,
        r,
        PageImageSource::Pixmap(&img),
        Default::default(),
    )
    .unwrap();

    // QR-code-like bitmap (middle): black/white checker
    let qr = make_checker(72, 72, [10, 10, 10], [245, 245, 245], 9);
    let q = Rect {
        x0: 72.0,
        y0: 470.0,
        x1: 144.0,
        y1: 542.0,
    };
    page.insert_image(
        &mut doc,
        q,
        PageImageSource::Pixmap(&qr),
        Default::default(),
    )
    .unwrap();
    shape = Shape::new(&mut page).unwrap();
    shape
        .insert_text(
            Point::new(150.0, 510.0),
            "QR (simulated bitmap)",
            &TextOptions::default(),
        )
        .unwrap()
        .commit(&mut doc, true)
        .unwrap();

    // Table (bottom): grid lines + cell text
    let gx0 = 72.0;
    let gy0 = 250.0;
    let gx1 = 400.0;
    let gy1 = 400.0;
    let rows = 4;
    let cols = 3;
    let cell_w = (gx1 - gx0) / cols as f32;
    let cell_h = (gy1 - gy0) / rows as f32;
    let mut shape = Shape::new(&mut page).unwrap();
    for i in 0..=rows {
        let y = gy0 + cell_h * i as f32;
        shape
            .draw_line(Point::new(gx0, y), Point::new(gx1, y))
            .unwrap();
    }
    for j in 0..=cols {
        let x = gx0 + cell_w * j as f32;
        shape
            .draw_line(Point::new(x, gy0), Point::new(x, gy1))
            .unwrap();
    }
    shape
        .finish(&FinishOptions::default())
        .unwrap()
        .commit(&mut doc, true)
        .unwrap();

    let cells: [[&str; 3]; 4] = [
        ["Name", "Age", "City"],
        ["Alice", "30", "Fuzhou"],
        ["Bob", "25", "Beijing"],
        ["Carol", "41", "Xiamen"],
    ];
    for (ri, row) in cells.iter().enumerate() {
        for (ci, txt) in row.iter().enumerate() {
            let x = gx0 + cell_w * ci as f32 + 6.0;
            let y = gy0 + cell_h * ri as f32 + cell_h * 0.65;
            let mut s = Shape::new(&mut page).unwrap();
            s.insert_text(Point::new(x, y), txt, &TextOptions::default())
                .unwrap()
                .commit(&mut doc, true)
                .unwrap();
        }
    }

    doc.save("/tmp/showcase.pdf").unwrap();

    // Render with our pipeline
    let vdoc = mupdf_mini_rs::ViewerDocument::open("/tmp/showcase.pdf").unwrap();
    println!("pages={}", vdoc.page_count());
    vdoc.save_page_png(0, 2.0, 0, "/tmp/showcase.png").unwrap();
    println!("rendered -> /tmp/showcase.png");
    println!("text extract:");
    println!("{}", vdoc.text_all().unwrap());
    let _ = Matrix::new_scale(1.0, 1.0); // keep import used
}
