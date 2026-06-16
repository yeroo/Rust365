# rust365

A fast, dependency-free converter from Microsoft Word `.docx` to HTML, in **Rust** —
a complete port of [Fast365](https://github.com/yeroo/Fast365) (C++). Everything is
from scratch: the ZIP reader, the DEFLATE decoder, the XML parser and the
WordprocessingML-to-HTML conversion. No external crates; standard library only.
Builds and runs on Windows and Linux.

```
rust365 <input.docx> [options]

  -o <file>      output path (default: input name with .html; "-" for stdout)
  --fragment     emit body content only, without the <html> wrapper
  --no-images    do not embed images
  --title <t>    override the document title
  --quiet        suppress the timing summary
  --version      print version and exit
```

## What is supported

A faithful port of Fast365, so the same coverage: paragraphs and headings, run
formatting (bold/italic/underline/strike/super/subscript/colour/highlight/caps/
hidden, character styles), alignment/indentation/shading/RTL, hyperlinks (external,
anchors, `w:fldSimple` and complex `HYPERLINK` fields, bookmarks), bullet/numbered
lists with nesting and `numbering.xml`, tables (colspan/rowspan via gridSpan/vMerge,
shading, header rows, nesting), footnotes/endnotes, images (DrawingML + VML, base64
data URIs), line breaks/tabs/symbols/text boxes/content controls/`mc:AlternateContent`,
and the document title from `docProps/core.xml`. Depth-limited recursion and
decompression caps guard against hostile input.

## Verification

rust365's HTML output is **byte-identical** to Fast365's. Validated on a corpus of
400 real-world `.docx` files: 400/400 produced identical output.

## Building & testing

```
cargo build --release
cargo test --release
```

## Layout

```
src/inflate.rs   DEFLATE (RFC 1951) decoder
src/zip.rs       ZIP central-directory reader (stored + deflate)
src/xml.rs       zero-copy XML pull parser
src/docx.rs      WordprocessingML -> HTML conversion
src/htmlutil.rs  escaping and base64
src/main.rs      CLI
```

## Why a Rust port?

The C++ Fast365 binary is sometimes flagged by Microsoft Defender as a false
positive (common for small, unsigned native tools). This is a clean re-implementation
in Rust. (A language change does not by itself prevent heuristic false positives —
code signing and submitting the binary to Microsoft are the durable fixes.)
