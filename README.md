# mupdf-mini-rs

A minimalist **MuPDF viewer** written in Rust, built on
[`mupdf-rs`](https://github.com/messense/mupdf-rs) (safe Rust bindings to
Artifex MuPDF). It is a from-scratch Rust reimagining of the spirit of
[MuPDF Mini](https://github.com/ArtifexSoftware/mupdf-android-viewer-mini):
open PDF / XPS / CBZ / EPUB documents and view them page by page with
keyboard-driven navigation, zoom, rotation, text extraction and single-page
PNG export.

## Features

- **Tiny dependency surface.** Core rendering uses only `mupdf` with the
  minimal `base14-fonts` feature (the URW fonts compiled into `mupdf-sys`),
  so there is no JS engine, OCR, or HTML layout pulled in.
- **Native GUI** via `winit` + `softbuffer` — zero high-level widget
  dependencies, the page is blitted straight from the MuPDF pixmap into the
  framebuffer.
- **CLI renderer** for automation / headless export to PNG.
- Page navigation, zoom (fit-to-width + free zoom), 90° rotation, text
  extraction, PNG export.

## Build

Requires a C/C++ toolchain, `libclang` (for bindgen) and `cmake` (to build
MuPDF from source). On macOS:

```sh
brew install cmake llvm   # llvm provides libclang
cargo build --release
```

## Usage

```sh
# Interactive viewer
cargo run -- view path/to/document.pdf

# Render a single page to PNG (headless)
cargo run -- render path/to/document.pdf --page 0 --scale 2.0 --out page.png
```

### Viewer controls

| Key / action            | Effect                          |
| ----------------------- | ------------------------------- |
| `←` / `→` (or `h`/`l`)  | Previous / next page            |
| `+` / `-`               | Zoom in / out                   |
| `0`                     | Reset zoom to 100%              |
| `w`                     | Fit page width to window        |
| `r`                     | Rotate 90° clockwise            |
| `/`                     | Search (type term, `Enter` runs, `Esc` cancels, `Backspace` edits) |
| `n` / `N`               | Next / previous search hit      |
| mouse wheel             | Zoom at cursor                  |
| `q` / `Esc`             | Quit                            |

Search uses ASCII line-editing (no IME); hits are highlighted in yellow on
the page, with the current hit drawn more strongly. For non-ASCII queries
pass `--search` via a future CLI flag or build with the document text path.

**Fonts.** By default the URW base14 fonts are compiled in (`base14-fonts`)
so Latin text renders. Enable the `system-fonts` feature to use platform
fonts (e.g. CJK) so documents with non-Latin scripts display correctly.

## Library

```rust
use mupdf_mini_rs::ViewerDocument;

let doc = ViewerDocument::open("doc.pdf")?;
println!("pages: {}", doc.page_count());
let page = doc.render(0, 2.0, 0)?;   // RGBA8 at 144 dpi
println!("{}x{}", page.width, page.height);
let text = doc.text(0)?;
```

## License

This project links against `mupdf-rs`, which is distributed under
**AGPL-3.0**. If you distribute a binary built from this code, the AGPL
obligations apply (notably the source-provision requirement). For closed /
proprietary distribution you must obtain a commercial MuPDF license from
Artifex.
