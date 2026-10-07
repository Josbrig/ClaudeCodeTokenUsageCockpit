// SPDX-License-Identifier: Apache-2.0
//! Odd and hostile input must never crash the parser, the history reader or the view model.

use chrono_tz::Europe::Berlin;
use cockpit_core::model::{Record, WindowSample};
use cockpit_core::parse::parse_status_line;
use cockpit_core::settings::Settings;
use cockpit_core::store;
use cockpit_core::viewmodel::{Inputs, ViewModel, WindowView, build};

const NOW_MS: i64 = 1_738_400_000_000;

fn parse(input: &str) -> Option<Record> {
    parse_status_line(input, NOW_MS).ok()
}

fn view(records: &[Record], now_ms: i64, last_error_ms: Option<i64>) -> ViewModel {
    let settings = Settings::default();
    build(&Inputs {
        now_ms,
        records,
        last_error_ms,
        load_error: None,
        settings: &settings,
        stats: None,
        tz: &Berlin,
    })
}

#[test]
fn req_108_parser_rejects_truncated_and_broken_json_without_panic() {
    let valid = r#"{"session_id":"s","rate_limits":{"five_hour":{"used_percentage":10,"resets_at":1738425600}}}"#;
    // Every prefix of a valid record is either an error or a record, never a panic.
    for end in 0..valid.len() {
        let _ = parse_status_line(&valid[..end], NOW_MS);
    }
    for input in [
        "", " ", "{", "}", "{\"a\":", "[", "null", "true", "12", "\"text\"", "{}}", "{'a':1}",
    ] {
        let result = parse_status_line(input, NOW_MS);
        if let Ok(record) = &result {
            assert_eq!(record.five_hour, None, "{input:?}");
        }
    }
}

#[test]
fn req_108_parser_ignores_values_of_the_wrong_type() {
    let inputs = [
        r#"{"rate_limits":[1,2,3]}"#,
        r#"{"rate_limits":"none"}"#,
        r#"{"rate_limits":{"five_hour":"yes","seven_day":42}}"#,
        r#"{"rate_limits":{"five_hour":{"used_percentage":"10","resets_at":1738425600}}}"#,
        r#"{"rate_limits":{"five_hour":{"used_percentage":10,"resets_at":"soon"}}}"#,
        r#"{"rate_limits":{"five_hour":{"used_percentage":null,"resets_at":null}}}"#,
        r#"{"rate_limits":{"five_hour":{"used_percentage":10,"resets_at":1.5}}}"#,
        r#"{"rate_limits":{"five_hour":{"used_percentage":[10],"resets_at":{"a":1}}}}"#,
        r#"{"session_id":7,"version":[1],"model":"Opus","context_window":5,"cost":"free"}"#,
    ];
    for input in inputs {
        let record = parse(input).unwrap_or_else(|| panic!("{input} should still parse"));
        assert_eq!(record.five_hour, None, "{input}");
        assert_eq!(record.seven_day, None, "{input}");
        assert_eq!(record.session_id, None, "{input}");
        assert_eq!(record.model, None, "{input}");
        assert_eq!(record.context_used_pct, None, "{input}");
        assert_eq!(record.cost_usd, None, "{input}");
    }
}

#[test]
fn req_108_parser_does_not_accept_nan_like_numbers() {
    // JSON has no NaN or Infinity; as text they are not numbers either.
    for number in [
        "NaN",
        "Infinity",
        "-Infinity",
        "\"NaN\"",
        "\"inf\"",
        "1e999",
        "-1e999",
    ] {
        let input = format!(
            r#"{{"rate_limits":{{"five_hour":{{"used_percentage":{number},"resets_at":1738425600}}}}}}"#
        );
        let record = parse(&input);
        if let Some(record) = record {
            assert_eq!(record.five_hour, None, "{number}");
        }
    }
}

#[test]
fn req_108_parser_survives_huge_numbers_and_deep_nesting() {
    let huge = r#"{"rate_limits":{"five_hour":{"used_percentage":1e300,"resets_at":9223372036854775807},
        "seven_day":{"used_percentage":0,"resets_at":-9223372036854775808}}}"#;
    let record = parse(huge).expect("huge numbers are still a record");
    assert_eq!(record.five_hour.unwrap().resets_at, i64::MAX);
    assert_eq!(record.seven_day.unwrap().resets_at, i64::MIN);
    assert_eq!(parse(r#"{"rate_limits":{"five_hour":{"used_percentage":1,"resets_at":18446744073709551615}}}"#).unwrap().five_hour, None);
    let deep = format!("{}1{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(parse_status_line(&deep, NOW_MS).is_err());
    let deep_object = format!("{}1{}", "{\"a\":".repeat(100_000), "}".repeat(100_000));
    assert!(parse_status_line(&deep_object, NOW_MS).is_err());
}

#[test]
fn req_108_the_view_model_survives_extreme_values() {
    for (used, resets_at) in [
        (1e300, i64::MAX),
        (0.0, i64::MIN),
        (100.0, i64::MAX),
        (-1.0, 0),
        (50.0, 1),
    ] {
        let sample = WindowSample {
            used_pct: used,
            resets_at,
        };
        let record = Record {
            received_at_ms: NOW_MS,
            session_id: None,
            cc_version: None,
            five_hour: Some(sample.clone()),
            seven_day: Some(sample),
            model: None,
            context_used_pct: None,
            cost_usd: None,
        };
        for now_ms in [NOW_MS, 0, i64::MAX / 1000, i64::MIN / 1000] {
            let _ = view(&[record.clone(), record.clone()], now_ms, None);
        }
    }
}

#[test]
fn req_108_history_reader_keeps_valid_lines_among_damaged_ones() {
    let dir = tempfile::tempdir().unwrap();
    let valid = |ms: i64| {
        format!(
            r#"{{"v":1,"received_at_ms":{ms},"five_hour":{{"used_pct":10.0,"resets_at":1738425600}}}}"#
        )
    };
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(valid(1).as_bytes());
    bytes.extend_from_slice(b"\n{ truncated line\n");
    bytes.extend_from_slice(valid(2).as_bytes());
    bytes.extend_from_slice(b"\n\xff\xfe\x00binary garbage\xc3\x28\n");
    bytes.extend_from_slice(br#"{"v":99,"received_at_ms":5}"#);
    bytes.extend_from_slice(b"\n");
    bytes.extend_from_slice(br#"{"v":1,"received_at_ms":"wrong type"}"#);
    bytes.extend_from_slice(b"\n\n   \n");
    bytes.extend_from_slice(valid(3).as_bytes());
    bytes.extend_from_slice(b"\n{\"v\":1,\"received_at_ms\":4"); // partial last line without a break
    std::fs::write(dir.path().join(store::HISTORY_FILE), bytes).unwrap();
    let read = store::read_history(dir.path()).unwrap();
    let received: Vec<i64> = read.records.iter().map(|r| r.received_at_ms).collect();
    assert_eq!(received, [1, 2, 3]);
    assert_eq!(
        read.skipped, 5,
        "truncated, binary, unknown version, wrong type, partial"
    );
}

#[test]
fn req_108_view_model_keeps_last_valid_values_marked_stale_after_a_malformed_record() {
    let good =
        parse(r#"{"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600}}}"#)
            .unwrap();
    let good_at_ms = good.received_at_ms;
    // The next input is malformed: the bridge writes only the marker, the records stay.
    assert!(parse("{ this is not json").is_none());
    let error_at_ms = good_at_ms + 10_000;
    let now_ms = good_at_ms + 20_000;
    let vm = view(&[good], now_ms, Some(error_at_ms));
    assert!(
        vm.stale,
        "a malformed record arrived after the last good one"
    );
    assert!(vm.age_text.starts_with("stale, "), "{}", vm.age_text);
    match &vm.five_hour {
        WindowView::Data(data) => assert_eq!(data.used_text, "23.5%", "the last good value stays"),
        WindowView::NoData { text } => panic!("the values must stay, got: {text}"),
    }
    assert_eq!(vm.banner, None);
}
