// SPDX-License-Identifier: Apache-2.0
//! The user guide must explain what the window shows (REQ-112): every message, label and number
//! format of the view model appears in `docs/user-guide.md`, and its links work. The example texts
//! in the guide are produced here by the real formatting functions, so a changed text breaks this
//! test until the guide is changed too.

use cockpit_core::format;
use cockpit_core::metrics::{Forecast, PaceState};
use cockpit_core::viewmodel::{
    BANNER_LOAD_ERROR, BANNER_NO_DATA_YET, BANNER_NO_LIMITS, BINDING_GLYPH, NO_DATA_GLYPH,
    SESSION_HEADING, STALE_GLYPH, STALE_LABEL, TOKEN_KINDS, TRANSCRIPTS_NOT_AVAILABLE,
    WINDOW_NO_DATA, WINDOW_RESET_PASSED, glyph, label,
};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn guide() -> String {
    std::fs::read_to_string(format!("{ROOT}/docs/user-guide.md"))
        .expect("docs/user-guide.md can be read")
        .replace("\r\n", "\n")
}

fn missing<'a>(guide: &str, texts: impl IntoIterator<Item = &'a String>) -> Vec<&'a String> {
    texts
        .into_iter()
        .filter(|text| !guide.contains(text.as_str()))
        .collect()
}

/// Short labels must stand out in the guide (bold, italic, code or the start of a table cell),
/// otherwise a chance hit in running text would make the test pass although nothing explains the
/// label. Long texts are specific enough to be looked for anywhere.
fn explained(guide: &str, text: &str) -> bool {
    if text.chars().count() >= 24 {
        return guide.contains(text);
    }
    [
        format!("**{text}**"),
        format!("**{text}:**"),
        format!("*{text}*"),
        format!("`{text}`"),
        format!("| {text}"),
    ]
    .iter()
    .any(|marked| guide.contains(marked.as_str()))
}

fn owned(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

#[test]
fn req_112_the_guide_explains_every_message_of_the_view_model() {
    let guide = guide();
    let mut texts = owned(&[
        BANNER_NO_DATA_YET,
        BANNER_NO_LIMITS,
        BANNER_LOAD_ERROR.trim_end_matches(' '),
        WINDOW_NO_DATA,
        WINDOW_RESET_PASSED,
        SESSION_HEADING,
        TRANSCRIPTS_NOT_AVAILABLE,
        STALE_LABEL,
        STALE_GLYPH,
        NO_DATA_GLYPH,
        BINDING_GLYPH,
    ]);
    for state in [PaceState::Under, PaceState::On, PaceState::Over] {
        texts.push(format!("*{}*", label(state)));
        texts.push(glyph(state).to_owned());
    }
    texts.extend(TOKEN_KINDS.iter().map(|kind| (*kind).to_owned()));
    assert_eq!(missing(&guide, &texts), Vec::<&String>::new());
}

#[test]
fn req_112_the_guide_shows_every_number_format_with_the_real_text() {
    let guide = guide();
    let texts = vec![
        format::age(12),
        format::stale(14 * 60),
        format::deviation(20.0),
        format::deviation(-20.0),
        format::factor(Some(1.5)),
        format::factor(None),
        format::rate(Some(20.0)),
        format::rate(Some(10.0)),
        format::unused(Some(40.0)),
        format::weekly(10, 6.0),
        format::forecast(&Forecast::LimitFirst { at_s: 3 * 3_600 }, 0),
        format::forecast(&Forecast::ResetFirst, 0),
        format::forecast(&Forecast::LimitReached, 0),
        format::forecast(&Forecast::NotAvailable, 0),
        format::pct(23.5),
        format::pct(42.0),
        format::usd(0.01),
        format::duration(3 * 3_600),
        format::duration(2 * 86_400 + 3 * 3_600),
        format!("cache share {}", format::pct(87.0)),
        "cache share not available".to_owned(),
        "tokens per 1% (estimate)".to_owned(),
        "not available (estimate)".to_owned(),
    ];
    assert_eq!(missing(&guide, &texts), Vec::<&String>::new());
}

/// Messages and labels of the window that are not constants and that the scan of
/// `crates/app/tests/user_guide_labels.rs` does not see (dialog results, messages). This list is kept by
/// hand: when such a text changes, change it here and in the guide. Short labels must be marked up
/// in the guide (see `explained`).
#[test]
fn req_112_the_guide_explains_the_hand_kept_list_of_labels_and_messages() {
    let guide = guide();
    let texts = owned(&[
        "5-hour window",
        "7-day window",
        "5h",
        "7d",
        "Yes",
        "No",
        "OK",
        "Change the Claude Code settings?",
        "The bridge is set up.",
        "The bridge is already set up; nothing to change.",
        "The Claude Code status line is not the bridge; nothing to change.",
        "Used",
        "Remaining",
        "Resets",
        "Target",
        "Deviation",
        "Pace factor",
        "Usage rate",
        "Forecast",
        "Unused at reset",
        "Recommended rate",
        "Binding limit",
        "Weekly plan",
        "This window's limit binds first.",
        "Context used",
        "Session cost",
        "Model",
        "Previous periods",
        "none yet",
        "Transcript statistics",
        "Per model",
        "Per day (newest first)",
        "Cache share",
        "Tokens per percentage point",
        "Chart shows",
        "Settings",
        "Set up bridge",
        "Remove bridge",
        "Remove everything",
        "Compact view (D)",
        "Detailed view (D)",
        "Licence notices",
        "Start with Windows",
        "Use this file",
        "The entry starts another file",
        "Switched off in the Windows list of startup apps",
        "Also delete the history, logs and settings",
        "usage-cockpit is already running.",
        "Tolerance",
        "Stale after",
        "Rate period",
        "Always on top",
        "Start view",
        "Save",
        "Cancel",
    ]);
    let unexplained: Vec<&String> = texts
        .iter()
        .filter(|text| !explained(&guide, text))
        .collect();
    assert_eq!(unexplained, Vec::<&String>::new());
}

/// The GitHub anchor of a heading: lower case, only letters, digits, spaces and hyphens, spaces
/// become hyphens.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

#[test]
fn req_112_the_links_of_the_guide_work() {
    let guide = guide();
    let anchors: Vec<String> = guide
        .lines()
        .filter(|line| line.starts_with('#'))
        .map(|line| anchor(line.trim_start_matches('#')))
        .collect();
    let mut broken = Vec::new();
    for part in guide.split("](").skip(1) {
        let Some(target) = part.split(')').next() else {
            continue;
        };
        if let Some(name) = target.strip_prefix('#') {
            if !anchors.iter().any(|a| a == name) {
                broken.push(target.to_owned());
            }
        } else if !target.starts_with("http") {
            let file = target.split('#').next().unwrap_or_default();
            if !std::path::Path::new(&format!("{ROOT}/docs/{file}")).exists() {
                broken.push(target.to_owned());
            }
        }
    }
    assert_eq!(broken, Vec::<String>::new());
}
