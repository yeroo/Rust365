//! rust365 — fast, dependency-free DOCX to HTML converter in Rust.
//! A port of Fast365 (C++).

mod docx;
mod htmlutil;
mod inflate;
mod xml;
mod zip;

use std::time::Instant;

use docx::{convert_docx_to_html, ConvertOptions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    eprintln!(
        "rust365 v{VERSION} - fast DOCX to HTML converter (no dependencies)\n\
\n\
Usage: rust365 <input.docx> [options]\n\
\n\
Options:\n\
\x20 -o <file>      output path (default: input name with .html; \"-\" for stdout)\n\
\x20 --fragment     emit body content only, without the <html> wrapper\n\
\x20 --no-images    do not embed images\n\
\x20 --title <t>    override the document title\n\
\x20 --quiet        suppress the timing summary\n\
\x20 --version      print version and exit"
    );
}

fn default_output_path(input: &str) -> String {
    let dot = input.rfind('.');
    let sep = input.rfind(['/', '\\']);
    if let Some(d) = dot {
        if sep.is_none() || d > sep.unwrap() {
            return format!("{}.html", &input[..d]);
        }
    }
    format!("{input}.html")
}

fn base_name(path: &str) -> String {
    let name = match path.rfind(['/', '\\']) {
        Some(s) => &path[s + 1..],
        None => path,
    };
    match name.rfind('.') {
        Some(d) if d > 0 => name[..d].to_string(),
        _ => name.to_string(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = String::new();
    let mut output = String::new();
    let mut opts = ConvertOptions::default();
    let mut quiet = false;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--version" => {
                println!("rust365 {VERSION}");
                return;
            }
            "--help" | "-h" => {
                print_usage();
                return;
            }
            "-o" if i + 1 < args.len() => {
                output = args[i + 1].clone();
                i += 1;
            }
            "--title" if i + 1 < args.len() => {
                opts.title = args[i + 1].clone();
                i += 1;
            }
            "--fragment" => opts.fragment = true,
            "--no-images" => opts.embed_images = false,
            "--quiet" | "-q" => quiet = true,
            _ if a.starts_with('-') && a != "-" => {
                eprintln!("rust365: unknown option '{a}'\n");
                print_usage();
                std::process::exit(2);
            }
            _ if input.is_empty() => input = a.to_string(),
            _ => {
                eprintln!("rust365: unexpected argument '{a}'");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    if input.is_empty() {
        print_usage();
        std::process::exit(2);
    }
    if output.is_empty() {
        output = default_output_path(&input);
    }
    if opts.title.is_empty() {
        opts.title = base_name(&input);
    }

    let t0 = Instant::now();
    let docx = match std::fs::read(&input) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("rust365: cannot read '{input}'");
            std::process::exit(1);
        }
    };

    let html = match convert_docx_to_html(&docx, &opts) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("rust365: {input}: {e}");
            std::process::exit(1);
        }
    };

    if output == "-" {
        use std::io::Write;
        let _ = std::io::stdout().write_all(html.as_bytes());
    } else if std::fs::write(&output, &html).is_err() {
        eprintln!("rust365: cannot write '{output}'");
        std::process::exit(1);
    }

    if !quiet {
        eprintln!(
            "rust365: {} ({} KB) -> {} ({} KB) in {:.1} ms",
            input,
            docx.len() / 1024,
            output,
            html.len() / 1024,
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}
