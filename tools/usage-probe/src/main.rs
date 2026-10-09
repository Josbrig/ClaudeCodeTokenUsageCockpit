// SPDX-License-Identifier: Apache-2.0
//! Test program for REQ-124: reads the 5-hour and 7-day usage percentages of the own Claude
//! subscription with the sign-in token that Claude Code stored on this computer.
//!
//! What it does, and what it never does:
//! - it reads the credentials file of Claude Code into memory, and uses the access token for the
//!   requests only; the token is never printed, never written and never refreshed;
//! - it asks the usage interface a few times (default 5 times, 60 seconds apart) and prints one line
//!   per request;
//! - it stops at the first answer that is not 200 (and honours `Retry-After` by not asking again);
//! - at the end it checks that the credentials file has the same bytes as before.
//!
//! Usage: `usage-probe [--calls N] [--interval SECONDS]`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// The undocumented interface behind `/usage` of Claude Code.
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// Header value that the interface expects together with the bearer token.
const BETA_HEADER: &str = "oauth-2025-04-20";
const DEFAULT_CALLS: u32 = 5;
const DEFAULT_INTERVAL_S: u64 = 60;
/// Never faster than this, whatever is asked for (REQ-124: minimum 15 seconds).
const MIN_INTERVAL_S: u64 = 15;

/// What is read from the credentials file. The token is kept out of `Debug` output on purpose.
struct Credentials {
    access_token: String,
    expires_at_ms: Option<i64>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_token", &"<hidden>")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// One window of the answer.
#[derive(Debug, PartialEq)]
struct Window {
    used_pct: f64,
    resets_at: String,
}

/// The figures of one answer.
#[derive(Debug, PartialEq)]
struct Usage {
    five_hour: Option<Window>,
    seven_day: Option<Window>,
}

/// The folder of Claude Code: `CLAUDE_CONFIG_DIR` if set, else `.claude` in the home folder.
fn claude_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".claude"))
}

/// Reads the access token and its expiry from the credentials file.
fn read_credentials(path: &Path) -> Result<Credentials, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read the credentials file: {e}"))?;
    parse_credentials(&bytes)
}

fn parse_credentials(bytes: &[u8]) -> Result<Credentials, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "the credentials file is not valid JSON".to_owned())?;
    let oauth = value
        .get("claudeAiOauth")
        .ok_or("the credentials file has no sign-in of a Claude account")?;
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or("the credentials file has no access token")?
        .to_owned();
    let expires_at_ms = oauth.get("expiresAt").and_then(Value::as_i64);
    Ok(Credentials {
        access_token,
        expires_at_ms,
    })
}

fn window(value: &Value, name: &str) -> Option<Window> {
    let w = value.get(name)?;
    Some(Window {
        used_pct: w.get("utilization")?.as_f64()?,
        resets_at: w.get("resets_at")?.as_str()?.to_owned(),
    })
}

/// The two windows of an answer; a missing or `null` window is `None`.
fn parse_usage(body: &str) -> Result<Usage, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "the answer is not valid JSON".to_owned())?;
    let usage = Usage {
        five_hour: window(&value, "five_hour"),
        seven_day: window(&value, "seven_day"),
    };
    if usage.five_hour.is_none() && usage.seven_day.is_none() {
        return Err("the answer has neither a five_hour nor a seven_day window".to_owned());
    }
    Ok(usage)
}

fn show(w: &Option<Window>) -> String {
    match w {
        Some(w) => format!("{:5.1} %  resets {}", w.used_pct, w.resets_at),
        None => "none".to_owned(),
    }
}

fn clock(unix_s: u64) -> String {
    let day = unix_s % 86_400;
    format!("{:02}:{:02}:{:02}Z", day / 3600, day % 3600 / 60, day % 60)
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The text of a line is checked against the token before it is printed: a token must never
/// reach the output, whatever the server answers.
fn say(token: &str, line: &str) {
    assert!(
        token.is_empty() || !line.contains(token),
        "refusing to print a line that contains the token"
    );
    println!("{line}");
}

struct Args {
    calls: u32,
    interval_s: u64,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        calls: DEFAULT_CALLS,
        interval_s: DEFAULT_INTERVAL_S,
    };
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| args.get(i + 1).ok_or(format!("{} needs a value", args[i]));
        match args[i].as_str() {
            "--calls" => {
                out.calls = value(i)?.parse().map_err(|_| "--calls needs a number")?;
                i += 2;
            }
            "--interval" => {
                out.interval_s = value(i)?
                    .parse()
                    .map_err(|_| "--interval needs a number of seconds")?;
                i += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if out.calls == 0 || out.calls > 20 {
        return Err("--calls must be from 1 to 20".to_owned());
    }
    if out.interval_s < MIN_INTERVAL_S {
        return Err(format!(
            "--interval must be at least {MIN_INTERVAL_S} seconds"
        ));
    }
    Ok(out)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args = match parse_args(&args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let Some(dir) = claude_dir() else {
        eprintln!("cannot find the folder of Claude Code");
        return ExitCode::from(2);
    };
    let path = dir.join(".credentials.json");
    let before = fs::read(&path).ok();
    let credentials = match read_credentials(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let token = credentials.access_token.clone();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();

    say(
        &token,
        &format!(
            "usage-probe: {} calls, {} s apart; interface {USAGE_URL}",
            args.calls, args.interval_s
        ),
    );
    let mut all_ok = true;
    for n in 1..=args.calls {
        let started = Instant::now();
        let at = now_s();
        let left = credentials
            .expires_at_ms
            .map(|ms| format!("{:.1} h", (ms as f64 / 1000.0 - at as f64) / 3600.0))
            .unwrap_or_else(|| "unknown".to_owned());
        let result = agent
            .get(USAGE_URL)
            .header("Authorization", &format!("Bearer {token}"))
            .header("anthropic-beta", BETA_HEADER)
            .header("Accept", "application/json")
            .header("User-Agent", "usage-probe")
            .call();
        let ms = started.elapsed().as_millis();
        match result {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let retry = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let body = response.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    match parse_usage(&body) {
                        Ok(u) => say(
                            &token,
                            &format!(
                                "{n}/{} {} HTTP {status} {ms} ms | 5h: {} | 7d: {} | token valid for {left}",
                                args.calls,
                                clock(at),
                                show(&u.five_hour),
                                show(&u.seven_day)
                            ),
                        ),
                        Err(e) => {
                            all_ok = false;
                            say(
                                &token,
                                &format!("{n}/{} {} HTTP 200 but {e}", args.calls, clock(at)),
                            );
                            break;
                        }
                    }
                } else {
                    all_ok = false;
                    let shown: String = body.chars().take(200).collect();
                    say(
                        &token,
                        &format!(
                            "{n}/{} {} HTTP {status} {ms} ms retry-after {} | token valid for {left} | {shown}",
                            args.calls,
                            clock(at),
                            retry.as_deref().unwrap_or("none")
                        ),
                    );
                    break;
                }
            }
            Err(e) => {
                all_ok = false;
                say(
                    &token,
                    &format!("{n}/{} {} no answer: {e}", args.calls, clock(at)),
                );
                break;
            }
        }
        if n < args.calls {
            sleep(Duration::from_secs(args.interval_s));
        }
    }

    let after = fs::read(&path).ok();
    let unchanged = before == after;
    say(
        &token,
        &format!(
            "credentials file {}",
            if unchanged {
                "unchanged (same bytes)"
            } else {
                "CHANGED during the run"
            }
        ),
    );
    if all_ok && unchanged {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ANSWER: &str = r#"{
        "five_hour": {"utilization": 37.0, "resets_at": "2026-10-08T18:09:59.566942+00:00", "limit_dollars": null},
        "seven_day": {"utilization": 62.0, "resets_at": "2026-10-14T03:59:59.567028+00:00"},
        "seven_day_opus": null,
        "extra_usage": {"is_enabled": false}
    }"#;

    #[test]
    fn req_124_the_two_windows_are_read_from_an_answer() {
        let u = parse_usage(SAMPLE_ANSWER).unwrap();
        assert_eq!(
            u.five_hour,
            Some(Window {
                used_pct: 37.0,
                resets_at: "2026-10-08T18:09:59.566942+00:00".to_owned()
            })
        );
        assert_eq!(u.seven_day.map(|w| w.used_pct), Some(62.0));
    }

    #[test]
    fn req_124_a_missing_or_null_window_is_none_and_unknown_fields_are_ignored() {
        let u = parse_usage(r#"{"five_hour": {"utilization": 5.5, "resets_at": "x"}, "seven_day": null, "new_field": 1}"#)
            .unwrap();
        assert!(u.five_hour.is_some());
        assert!(u.seven_day.is_none());
    }

    #[test]
    fn req_124_an_answer_without_a_window_or_not_json_is_an_error() {
        assert!(parse_usage(r#"{"seven_day_opus": null}"#).is_err());
        assert!(parse_usage("not json").is_err());
        assert!(parse_usage("").is_err());
    }

    #[test]
    fn req_124_the_token_is_read_from_the_credentials_and_never_shown() {
        let c = parse_credentials(
            br#"{"claudeAiOauth": {"accessToken": "TOKEN-FOR-TEST", "refreshToken": "REFRESH-FOR-TEST", "expiresAt": 1791000000000}}"#,
        )
        .unwrap();
        assert_eq!(c.access_token, "TOKEN-FOR-TEST");
        assert_eq!(c.expires_at_ms, Some(1_791_000_000_000));
        let shown = format!("{c:?}");
        assert!(!shown.contains("TOKEN-FOR-TEST"), "{shown}");
        assert!(!shown.contains("REFRESH-FOR-TEST"), "{shown}");
    }

    #[test]
    fn req_124_missing_or_empty_credentials_are_reported_not_guessed() {
        assert!(parse_credentials(b"{}").is_err());
        assert!(parse_credentials(br#"{"claudeAiOauth": {}}"#).is_err());
        assert!(parse_credentials(br#"{"claudeAiOauth": {"accessToken": ""}}"#).is_err());
        assert!(parse_credentials(b"no json").is_err());
        assert!(read_credentials(Path::new("this/file/does/not/exist.json")).is_err());
    }

    #[test]
    fn req_124_the_credentials_file_is_read_and_left_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".credentials.json");
        let original = br#"{"claudeAiOauth": {"accessToken": "TOKEN-FOR-TEST"}}"#;
        fs::write(&path, original).unwrap();
        let c = read_credentials(&path).unwrap();
        assert_eq!(c.access_token, "TOKEN-FOR-TEST");
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    #[should_panic(expected = "refusing to print")]
    fn req_124_a_line_with_the_token_is_never_printed() {
        say("TOKEN-FOR-TEST", "something TOKEN-FOR-TEST something");
    }

    #[test]
    fn req_124_the_arguments_are_limited() {
        let a = |v: &[&str]| parse_args(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let ok = a(&[]).unwrap();
        assert_eq!((ok.calls, ok.interval_s), (5, 60));
        assert!(a(&["--calls", "21"]).is_err());
        assert!(a(&["--calls", "0"]).is_err());
        assert!(a(&["--interval", "14"]).is_err());
        assert!(a(&["--interval", "15", "--calls", "3"]).is_ok());
        assert!(a(&["--other"]).is_err());
        assert!(a(&["--calls"]).is_err());
    }

    #[test]
    fn req_124_the_clock_shows_the_time_of_day() {
        assert_eq!(clock(0), "00:00:00Z");
        assert_eq!(clock(86_399), "23:59:59Z");
        assert_eq!(clock(3_661), "01:01:01Z");
    }
}
