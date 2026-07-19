mod document;
mod error;
mod render;
mod viewer;

use std::process::exit;

fn print_help() {
    eprintln!("mupdf-mini-rs — a minimalist MuPDF viewer (powered by mupdf-rs)");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mupdf-mini-rs view   <file>            Open the interactive viewer");
    eprintln!("  mupdf-mini-rs render <file> [options]  Render a page and save as PNG");
    eprintln!();
    eprintln!("RENDER OPTIONS:");
    eprintln!("  --page   N   page index, 0-based (default 0)");
    eprintln!("  --scale  F   zoom factor, 1.0 = 72dpi (default 2.0)");
    eprintln!("  --rotate D   rotation in degrees 0/90/180/270 (default 0)");
    eprintln!("  --out    P   output PNG path (default page.png)");
    eprintln!("  --search S   highlight all matches of S (ASCII or via argv; e.g. CJK)");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        print_help();
        exit(1);
    }
    let cmd = args[1].as_str();
    let file = args[2].clone();

    match cmd {
        "render" => {
            let mut page = 0usize;
            let mut scale = 2.0f32;
            let mut rotation = 0u8;
            let mut out = "page.png".to_string();
            let mut search: Option<String> = None;
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--page" => {
                        page = args[i + 1].parse().expect("invalid --page");
                        i += 2;
                    }
                    "--scale" => {
                        scale = args[i + 1].parse().expect("invalid --scale");
                        i += 2;
                    }
                    "--rotate" => {
                        rotation = args[i + 1].parse().expect("invalid --rotate");
                        i += 2;
                    }
                    "--out" => {
                        out = args[i + 1].clone();
                        i += 2;
                    }
                    "--search" => {
                        search = Some(args[i + 1].clone());
                        i += 2;
                    }
                    other => {
                        eprintln!("unknown option: {other}");
                        exit(1);
                    }
                }
            }
            match document::ViewerDocument::open(&file).and_then(|d| {
                d.save_page_png_with_search(page, scale, rotation, search.as_deref(), &out)
            }) {
                Ok(()) => eprintln!("rendered page {page} -> {out}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            }
        }
        "view" => match viewer::run(&file) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: {e}");
                exit(1);
            }
        },
        _ => {
            print_help();
            exit(1);
        }
    }
}
