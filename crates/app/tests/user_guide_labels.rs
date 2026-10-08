// SPDX-License-Identifier: Apache-2.0
//! The user guide must name every button, heading, row label and box of the window (REQ-112).
//! The labels are read from the source of the window (`src/gui/*.rs`), so a new label that the guide
//! does not explain breaks this test; no hand-kept list can fall behind.

use std::fs;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// The calls whose first or last string literal is a text the person reads.
const PATTERNS: &[&str] = &[
    "pub const LABEL: &str = \"",
    "ui.button(\"",
    "Button::new(\"",
    "heading(ui, \"",
    "row(ui, \"",
    "hyperlink_to(\"",
    "ui.label(\"",
    "Window::new(\"",
    "ui.checkbox(&mut draft.always_on_top, \"",
    ".checkbox(&mut delete, \"",
    "ui.radio_value(&mut draft.start_view, StartView::Compact, \"",
    "ui.radio_value(&mut draft.start_view, StartView::Detailed, \"",
];

/// Texts that are read from the source but need no explanation of their own (none so far).
const NOT_EXPLAINED: &[&str] = &[];

fn literal_after(source: &str, start: usize) -> Option<String> {
    let rest = &source[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Every label that the patterns find in `src/gui`, with the file it comes from.
fn labels() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let folder = format!("{ROOT}/src/gui");
    let mut files: Vec<std::path::PathBuf> = fs::read_dir(&folder)
        .expect("src/gui can be read")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    // the text of the box of the start entry that earlier versions had is a constant of this file
    files.push(std::path::PathBuf::from(format!("{ROOT}/src/autostart.rs")));
    for path in files {
        let source = fs::read_to_string(&path).expect("source can be read");
        // Only the code, not the tests at the end of the file.
        let code = source.split("#[cfg(test)]").next().unwrap_or_default();
        for pattern in PATTERNS {
            let mut from = 0;
            while let Some(at) = code[from..].find(pattern) {
                let mut begin = from + at + pattern.len();
                if pattern.ends_with('=') {
                    // the literal starts at the next quote
                    match code[begin..].find('"') {
                        Some(quote) => begin += quote + 1,
                        None => break,
                    }
                }
                if let Some(text) = literal_after(code, begin) {
                    found.push((
                        text,
                        path.file_name().unwrap().to_string_lossy().into_owned(),
                    ));
                }
                from = begin;
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn guide() -> String {
    fs::read_to_string(format!("{ROOT}/../../docs/user-guide.md"))
        .expect("docs/user-guide.md can be read")
        .replace("\r\n", "\n")
}

#[test]
fn req_112_the_scan_finds_the_labels_of_the_window() {
    let labels: Vec<String> = labels().into_iter().map(|(text, _)| text).collect();
    // A few that must be found, so that the scan cannot silently find nothing.
    for expected in [
        "Set up bridge",
        "Remove bridge",
        "Remove everything",
        "Compact view (D)",
        "Used",
        "Forecast",
        "Binding limit",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "the scan did not find {expected:?}: {labels:?}"
        );
    }
}

#[test]
fn req_112_the_guide_names_every_label_of_the_window() {
    let guide = guide();
    let missing: Vec<(String, String)> = labels()
        .into_iter()
        .filter(|(text, _)| !NOT_EXPLAINED.contains(&text.as_str()))
        .filter(|(text, _)| !guide.contains(text.as_str()))
        .collect();
    assert_eq!(missing, Vec::<(String, String)>::new());
}
