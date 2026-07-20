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
  `base14-fonts` + `system-fonts` features, so there is no JS engine, OCR,
  or HTML layout pulled in. `system-fonts` lets non-Latin scripts (e.g. CJK)
  render using platform fonts.
- **Native GUI** via `winit` + `softbuffer` — zero high-level widget
  dependencies, the page is blitted straight from the MuPDF pixmap into the
  framebuffer.
- **CLI renderer** for automation / headless export to PNG, with optional
  search-term highlighting.
- Page navigation, zoom (fit-to-width + free zoom), 90° rotation, text
  extraction, PNG export.
- In-viewer and headless **text search** with yellow hit highlighting.

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

# Render every page (page-0.png, page-1.png, ...)
cargo run -- render path/to/document.pdf --all --out page

# Render with search-term highlighting (non-ASCII queries work here,
# bypassing the viewer's ASCII-only search box)
cargo run -- render path/to/document.pdf --page 0 --search "Hello" --out hi.png

# Extract the full document text to a file
cargo run -- render path/to/document.pdf --text doc.txt
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

Search in the viewer uses ASCII line-editing (no IME); hits are highlighted
in yellow on the page, with the current hit drawn more strongly. For
non-ASCII queries (e.g. CJK), use the `--search TERM` CLI flag, which takes
the term from argv and therefore bypasses the ASCII-only input box.

## Testing

The suite is fully self-contained — fixtures are generated in-memory with
MuPDF's `Shape` API, so there are no external files or network calls.

```sh
cargo test            # unit + integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Coverage includes: page render at multiple zooms/rotations, text extraction,
PNG export, in-bounds search-hit quads, yellow highlight injection, the
`ctm_for` transform matrix (scale/rotation orthogonality), `blit_to_buffer`
centering and out-of-bounds guards, CJK render path, and multi-page
out-of-range errors.

## CI

A GitHub Actions pipeline (`.github/workflows/build.yml`) runs `fmt`,
`clippy -D warnings`, `build` and `test` on `ubuntu-latest` for every push
to `main` and every pull request.

**Fonts.** By default the URW base14 fonts are compiled in (`base14-fonts`)
so Latin text renders. The `system-fonts` feature is also enabled by default
so documents with non-Latin scripts (e.g. CJK) display correctly.

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
