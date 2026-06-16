//! Smoke test: convert the sample .docx and check the HTML is well-formed.
//! Cargo sets CARGO_BIN_EXE_rust365 for integration tests of the binary.

use std::process::Command;

fn convert(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rust365"))
        .args(args)
        .output()
        .expect("run rust365");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn sample_converts_to_html() {
    let sample = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample.docx");
    let (ok, html) = convert(&[sample, "-o", "-", "--quiet"]);
    assert!(ok, "conversion failed");
    assert!(html.contains("<!DOCTYPE html>"), "missing doctype");
    assert!(html.contains("<body>"), "missing body");
    assert!(html.trim_end().ends_with("</html>"), "missing closing html");
    // balanced top-level wrapper
    assert_eq!(html.matches("<html>").count(), 1);
    assert_eq!(html.matches("</html>").count(), 1);
}

#[test]
fn fragment_has_no_wrapper() {
    let sample = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample.docx");
    let (ok, html) = convert(&[sample, "-o", "-", "--fragment", "--quiet"]);
    assert!(ok);
    assert!(!html.contains("<html>"), "fragment should have no <html> wrapper");
    assert!(!html.contains("<!DOCTYPE"), "fragment should have no doctype");
}

#[test]
fn rejects_non_docx() {
    // a non-ZIP byte blob must fail gracefully, not panic
    let dir = std::env::temp_dir().join("rust365_bad.docx");
    std::fs::write(&dir, b"this is not a zip file at all").unwrap();
    let (ok, _) = convert(&[dir.to_str().unwrap(), "-o", "-", "--quiet"]);
    assert!(!ok, "garbage input should be rejected");
    let _ = std::fs::remove_file(&dir);
}
