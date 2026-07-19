//! mupdf-mini-rs — a minimalist MuPDF viewer built on `mupdf-rs`.
//!
//! The library exposes a small, safe surface over MuPDF's rendering
//! pipeline: open a document, render a page to an RGBA pixmap at a given
//! zoom/rotation, extract text, and save a page as PNG.

pub mod document;
pub mod error;
pub mod render;
pub mod viewer;

pub use document::ViewerDocument;
pub use error::{Error, Result};
pub use render::RenderedPage;
