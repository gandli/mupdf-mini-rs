use mupdf::{Colorspace, Document, Matrix, TextExtractOptions};

use crate::error::{Error, Result};
use crate::render::RenderedPage;

/// A loaded document with a safe, page-oriented rendering API.
///
/// Wraps `mupdf::Document` and keeps the page count cached so callers can
/// validate indices before loading a page.
pub struct ViewerDocument {
    doc: Document,
    page_count: usize,
}

impl ViewerDocument {
    /// Open a document from a file path.
    pub fn open(path: &str) -> Result<Self> {
        let doc = Document::open(path)?;
        let page_count = doc.page_count()? as usize;
        if page_count == 0 {
            return Err(Error::EmptyDocument);
        }
        Ok(Self { doc, page_count })
    }

    /// Number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Extract the plain text of a page.
    #[allow(dead_code)]
    pub fn text(&self, index: usize) -> Result<String> {
        let page = self.load_page(index)?;
        Ok(page.text(TextExtractOptions::default())?)
    }

    /// Search a page for `needle`, returning hit quads in PDF coordinates.
    pub fn search(&self, index: usize, needle: &str) -> Result<Vec<mupdf::Quad>> {
        let page = self.load_page(index)?;
        let hits = page.search(needle, 200)?;
        Ok(hits.iter().cloned().collect())
    }

    /// Page size in PDF points (1/72 inch), before any zoom/rotation.
    pub fn page_size_pt(&self, index: usize) -> Result<(f32, f32)> {
        let page = self.load_page(index)?;
        let r = page.bounds()?;
        Ok(((r.x1 - r.x0), (r.y1 - r.y0)))
    }

    fn load_page(&self, index: usize) -> Result<mupdf::Page> {
        if index >= self.page_count {
            return Err(Error::PageOutOfRange {
                index,
                count: self.page_count,
            });
        }
        Ok(self.doc.load_page(index as i32)?)
    }

    /// Render a page to an RGBA pixmap.
    ///
    /// * `scale` — zoom factor; `1.0` renders at 72 dpi, `2.0` at 144 dpi.
    /// * `rotation` — clockwise rotation in degrees (typically `0`/`90`/`180`/`270`).
    pub fn render(&self, index: usize, scale: f32, rotation: u8) -> Result<RenderedPage> {
        let pixmap = self.render_pixmap(index, scale, rotation)?;
        let width = pixmap.width() as usize;
        let height = pixmap.height() as usize;
        let rgba = pixmap.samples().to_vec();
        Ok(RenderedPage {
            width,
            height,
            rgba,
        })
    }

    /// Render a page and write it to a PNG file.
    pub fn save_page_png(
        &self,
        index: usize,
        scale: f32,
        rotation: u8,
        out: &str,
    ) -> Result<()> {
        let pixmap = self.render_pixmap(index, scale, rotation)?;
        pixmap.save_as(out, mupdf::ImageFormat::PNG)?;
        Ok(())
    }

    fn render_pixmap(
        &self,
        index: usize,
        scale: f32,
        rotation: u8,
    ) -> Result<mupdf::Pixmap> {
        let page = self.load_page(index)?;
        let mut ctm = Matrix::new_scale(scale, scale);
        if rotation != 0 {
            // `concat` mutates `ctm` in place and returns `&mut Self`.
            ctm.concat(Matrix::new_rotate(rotation as f32));
        }
        let pixmap = page.to_pixmap(&ctm, &Colorspace::device_rgb(), true, true)?;
        Ok(pixmap)
    }
}
