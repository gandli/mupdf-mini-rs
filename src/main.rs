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
    eprintln!("  --all        render every page to <out-prefix>-<n>.png");
    eprintln!("  --text   T   extract full document text to file T");
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
            let mut all = false;
            let mut search: Option<String> = None;
            let mut text_out: Option<String> = None;
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
                    "--all" => {
                        all = true;
                        i += 1;
                    }
                    "--text" => {
                        text_out = Some(args[i + 1].clone());
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
            let doc = match document::ViewerDocument::open(&file) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            };
            if let Some(t) = text_out {
                match doc.text_all() {
                    Ok(text) => match std::fs::write(&t, text) {
                        Ok(()) => eprintln!("extracted text -> {t}"),
                        Err(e) => {
                            eprintln!("error writing {t}: {e}");
                            exit(1);
                        }
                    },
                    Err(e) => {
                        eprintln!("error: {e}");
                        exit(1);
                    }
                }
                return;
            }
            if all {
                match doc.save_all_pages_png_with_search(scale, rotation, search.as_deref(), &out) {
                    Ok(paths) => {
                        for p in &paths {
                            eprintln!("rendered -> {p}");
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        exit(1);
                    }
                }
            } else {
                match doc.save_page_png_with_search(page, scale, rotation, search.as_deref(), &out)
                {
                    Ok(()) => eprintln!("rendered page {page} -> {out}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        exit(1);
                    }
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
