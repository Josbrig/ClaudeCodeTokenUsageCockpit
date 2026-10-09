# Concept: usage percentages from the account (REQ-124)

This concept describes how the cockpit reads the 5-hour and 7-day usage percentages of the owner's own Claude subscription with the sign-in token that Claude Code has stored on the same computer. It builds on the requirements [REQ-124](requirements.md), [REQ-103](requirements.md), [REQ-104](requirements.md), [REQ-009](requirements.md), [REQ-017](requirements.md) and [REQ-022](requirements.md), on the research in [research/data-sources.md](research/data-sources.md) and [research/token-dimensions-and-terms.md](research/token-dimensions-and-terms.md), and on the test program [tools/usage-probe](../tools/usage-probe/README.md).

## 1. Goal and limits

**Goal.** The percentages of both windows are shown live while the cockpit runs, without a terminal session of Claude Code and for every way of working with Claude Code (terminal, VS Code extension, other computers of the same account).

**Not goals.** Absolute token limits (not published), other accounts than the owner's, a sign-in of the cockpit of its own, renewing the token, any other use of the interface, a replacement of the status line (it stays as a source, see section 4).

**Facts the design rests on.**
- Measured on the owner's computer on 2026-10-08: `GET https://api.anthropic.com/api/oauth/usage` with the headers `Authorization: Bearer <access token>` and `anthropic-beta: oauth-2025-04-20` answered HTTP 200 in 232 to 329 ms, six times (five of them by `usage-probe`, 60 seconds apart). The answer holds `five_hour` and `seven_day`, each with `utilization` (percent) and `resets_at` (ISO 8601, UTC, with fractions of a second that vary by less than one second between calls).
- Measured: the credentials file (`%USERPROFILE%\.claude\.credentials.json`) holds a block `claudeAiOauth` with `accessToken`, `expiresAt`, `scopes` (including `user:profile`) and more. The access token was valid for 7.6 hours after Claude Code had renewed it, and for 0.3 hours some hours earlier. The file had the same bytes after every run of `usage-probe`.
- Measured: the access token is renewed by Claude Code, not by the interface or the test program, when Claude Code is used.
- Not documented by Anthropic: the interface, its headers and its fields. It can change or disappear at any time.
- Terms of use: no explicit permission and no explicit ban of this use was found; the general clause on access through automated means applies by its letter (details in the research note). This is decision D3 of REQ-124 and is why the whole feature is behind a gate (section 3).

## 2. Overview

```
 credentials file of Claude Code            Anthropic servers
 (read only, per request)                   api.anthropic.com
        |                                          ^
        v                                          | one GET per interval
 +----------------+    token (memory only)   +-----+-----------+
 | CredentialRead |------------------------->|  UsageClient    |
 +----------------+                          +-----+-----------+
                                                   | JSON text
                                                   v
                                      +--------------------------+
                                      | core::account (pure)     |
                                      | parse_usage, state       |
                                      | machine, to_record       |
                                      +------------+-------------+
                                                   | Record + AccountStatus
                    +------------------------------+--------------------+
                    v                                                   v
          history (record only when                         window state (in memory)
          the values changed)                               status, age, source
                    |                                                   |
                    +------------------> view model <-------------------+
                                  (rows, detailed view, texts)
```

The code is split on purpose: everything that decides something is in `cockpit-core` and has no input or output (parsing, the state machine, the conversion to a record, the texts); everything that touches the network or the file system is in the app crate behind one narrow interface. The poller is a plain `std` thread like the existing ones (no async runtime), and runs only while the cockpit window runs.

## 3. The gate: a switch that is off, in code and at run time

Because the terms position is the owner's (D3), the feature has two locks.

1. **Cargo feature `account-usage`** in the app crate. It holds the HTTP client and the poller. Until D3 is decided, the feature is **off by default**: the released executable contains no network code at all, which also makes REQ-103 checkable by `cargo tree` (no HTTP crate). The feature is built and tested with `--features account-usage`. Turning the default on is a one-line change that the owner approves with D3.
2. **Setting `account.enabled`** (default `false`) in `settings.toml`, switched by a box in the settings dialog. Switching it on for the first time shows a consent dialog with: what the cockpit will do (read the sign-in token that Claude Code stored, ask `api.anthropic.com` for the usage figures every N seconds, nothing else, never change or renew the token), the clause of the Consumer Terms on access through automated means quoted word for word, the sentence that Anthropic may refuse or act without notice, and the buttons *Enable* and *Cancel*. The consent is stored as `account.consent_version = 1` and a time; a changed consent text asks again.

No other path reaches the network. Switching the box off stops the thread within one second and no request is sent afterwards.

## 4. Sources and the rule between them

The cockpit has three sources of percentages and uses the **newest** value of each window, whichever source it comes from (REQ-017 extended):

| Source | Gives | Frequency | Needs |
|---|---|---|---|
| Account (this concept) | both windows | every interval (default 60 s) | gate open, sign-in file, valid token, network |
| Status line (bridge) | both windows | after every answer in a terminal | bridge set up, terminal client |
| Transcript files | tokens, no percentages | live (REQ-122) | none |

A record gets the field `source` (`"statusline"` or `"account"`; a record without it counts as `"statusline"`; REQ-115 allows the addition because readers ignore unknown fields and the format version stays 1). All calculations (periods, rate, forecast, pace, charts) work on the mixed history unchanged, because both sources deliver the same two numbers per window. Both sources carry the figures of the same account; the two can differ for a short time because they are read at different moments.

The window-reset rule (REQ-022) is unchanged. Account records bring the reset time with a sub-second jitter; the existing tolerance of 60 seconds for "same period" absorbs it.

## 5. The token

Rules (REQ-104 as amended):
- The credentials file is opened read only, once per request, with sharing allowed (Claude Code writes it). The program never writes, moves or deletes it.
- Only `claudeAiOauth.accessToken` and `claudeAiOauth.expiresAt` are used. `refreshToken` and everything else are not read into any structure.
- The token lives in a type `Secret` that has no `Display`, no `Serialize` and a `Debug` that prints `<hidden>`; it is passed to the client by reference and dropped after the request. No log line, error text, status text or file contains it or any part of it.
- The request is sent as what it is: `User-Agent: usage-cockpit/<version>`; the program does not present itself as Claude Code.
- The token is never renewed. If `expiresAt` is in the past (with 30 seconds of margin) the request is not sent (state *token expired*). The credentials file is looked at again only through its size and modification time every 30 seconds; a change triggers the next request, because Claude Code has renewed the token.
- Where the file does not exist or has no sign-in block, the state is *no sign-in*; there is no search for another place. Windows is the only system with a measured location. Linux is assumed to use the same relative path; on macOS the token may be in the key chain, which is a separate decision and ticket.

## 6. Polling and the state machine

Pure code in `core::account`, driven by a clock and by "events" (`Tick`, `Answer(status, retry_after, body)`, `NoAnswer(error)`, `CredentialsChanged`, `Disabled`), so that every transition is a unit test with a fake clock.

| State | Meaning | Next request |
|---|---|---|
| Off | gate closed or box off | never |
| Waiting | normal operation | at `interval` (default 60 s, min 15 s, REQ-124) |
| Ok(checked_at) | last answer 200 and understood | as Waiting |
| TokenExpired | `expiresAt` in the past, or answer 401 | only after `CredentialsChanged` |
| Refused | answer 403, or a second 401 right after a change | not before one hour has passed or `CredentialsChanged`; one line in the log |
| RateLimited(until) | answer 429 | after `Retry-After` (300 s if the header is missing), then doubling up to 15 minutes |
| NoNetwork | no answer, timeout (15 s) | backoff 1, 2, 4 … up to 10 minutes |
| NotUnderstood | answer 200 but no window found | as Waiting; one log line per change of the cause; last values stay |
| NoSignIn | file or block missing | file check every 30 s |

Requests are sequential, one at a time. A new request never starts before the previous one has ended. After any non-success the last values stay on the screen, marked as stale by REQ-009 once they are older than the stale threshold, and the detailed view says which state it is in.

## 7. What is stored

- A record is appended to the history only **when a value or a reset time changed** since the last account record, plus one keep-alive record per hour. At one request per minute this is a few hundred records a day instead of 1,440, and the history stays small even without a time limit (REQ-025 as amended).
- The time of the last successful check and the state are kept in memory and shown; they are not stored.
- Nothing about the token is stored anywhere: not in the history, the database (REQ-121), the settings, the log or the dump of an error. *Remove everything* has nothing extra to remove for this feature besides the settings entries.
- With the database of REQ-121 the records go into its table of account records like the status line records.

## 8. Display

- Rows and charts: unchanged; they show the newest value, from whichever source.
- Compact view: no new element; the data age (REQ-009) is that of the newest record of any source.
- Detailed view, a line below the windows: *Source: account, checked 12 s ago* or *Source: status line, 4 min ago*; in a state other than Ok the line names the state in one sentence. The wording is fixed as constants in the view model and each sentence is explained in the user guide (the guide tests demand it):
  - *Account query is off.* · *No Claude Code sign-in found on this computer.* · *The sign-in token has expired; it is renewed when Claude Code is used.* · *Anthropic refused the request; the query is paused.* · *Waiting for the server (rate limit), next request at 14:32.* · *No connection to Anthropic.* · *The answer of the server was not understood (the interface may have changed).*
- Settings dialog: the box *Read the account usage with the sign-in of Claude Code*, the interval field (seconds, 15 to 3600).

## 9. The HTTP client

| Option | Measured or known | Verdict |
|---|---|---|
| `ureq` 3 with `rustls` (ring) and `webpki-roots` | Built into `usage-probe`: executable 2.5 MB with `std` and `serde_json` (the cockpit is 7.2 MB); 41 crates in the tree; works with the static C runtime on Windows; needs a C compiler to build `ring` (present with the MSVC tools and on Linux and macOS). Licences: all accepted by `deny.toml` except `webpki-roots` (CDLA-Permissive-2.0, the licence of the list of root certificates). | First realization. Needs either CDLA-Permissive-2.0 in the allowed licences (owner's confirmation) or the next row. |
| `ureq` 3 with the certificate check of the operating system (`rustls-platform-verifier`) | Not measured. Removes the root list and its licence; adds the verifier crates. | Candidate to replace the first row after a measurement of size and licences. |
| WinHTTP through `windows-sys` | Not measured. No new crate on Windows, uses the system certificate store; Windows only, Linux and macOS would need own code. | Not chosen: the other systems would need a second implementation. |

The client is wrapped by a trait `UsageClient { fn get(&self, token: &Secret) -> Result<Reply, ClientError> }`; the poller and all tests use the trait. Only the implementation with `ureq` lives behind the Cargo feature and is the only place with network code. The URL is a constant; there is no setting for it.

## 10. Tests and checks

| What | How |
|---|---|
| Parsing of the answer (both windows, one window, `null` window, unknown fields, not JSON, empty) | unit tests in `core`, names `req_124_*`; the sample answer of the measurement above is the fixture |
| Credentials block (token, expiry, missing, empty, not JSON) | unit tests; the token never appears in `Debug` |
| The state machine of section 6 with a fake clock | unit tests per transition, including 429 with and without `Retry-After`, 401 then change, 403 pause, backoff growth and cap, interval minimum |
| Poller and client against a fake server on `127.0.0.1` | integration test with the `ureq` implementation (feature on): 200, 401, 403, 429 with `Retry-After`, a body that is not understood, a timeout |
| Token never leaks | integration test: fake credentials file with a marker token; after a run with answers and errors, the data folder, log, settings, history and all captured output are searched for the marker (none); the credentials file has the same bytes |
| Credentials file untouched | byte comparison before and after, as in `usage-probe` |
| Network use only to Anthropic (REQ-103) | `cargo tree` and a search of the source for other network code; a manual check of the open connections during a ten-minute run lists only the addresses of `api.anthropic.com` |
| Equality with `/usage` within 1 percentage point (REQ-124) | manual: the owner reads `/usage` in Claude Code while the cockpit shows the account value; the result is recorded in the traceability |
| Live without a terminal session (REQ-124) | manual: work in the VS Code extension, watch the percentage change in the window at the next interval |
| Idle CPU below 1 % and memory below 100 MB (REQ-105) | the existing measurement script with the feature on |

## 11. Realization in tickets (proposal, one closable thing each, Windows first)

1. `core::account`: parsing, `Secret`, credentials block, `AccountStatus`, state machine, the texts; unit tests. No I/O.
2. App crate: credentials reader (file, size and time watch) and the trait `UsageClient` with a fake for tests.
3. App crate, feature `account-usage`: the `ureq` implementation, the fake server tests, the check that no token leaks.
4. Poller thread and merge into the window state; history append on change plus keep-alive; `Record.source`.
5. Settings: `account.enabled`, interval, consent dialog, settings box; guide labels.
6. Detailed view: source and state line.
7. Documentation: user guide (what it does, the risk, how to switch off), technical documentation, REQ-103 and REQ-104 sentences in the guide, the technical documentation, the development guide and the README ("no network connection of its own" changes to "none, unless the account query is switched on"), the licence list, the traceability.
8. Manual verification run (equality with `/usage`, live without a terminal, ten-minute connection check, idle measurement) and its record.
9. Linux and macOS: location and form of the stored sign-in (key chain on macOS), after the Windows version works.

Tickets 1 to 3 do not touch the network of the released program (the feature is off by default). Ticket 5 needs the wording of the consent text from the owner.

## 12. Open decisions

- **D3** (terms of use position; unchanged): the owner's. The two locks of section 3 are built to hold until it is decided; permission from Anthropic or the owner's written decision opens them.
- Whether `CDLA-Permissive-2.0` joins the allowed licences or the client uses the certificate check of the operating system.
- Default interval (proposal 60 s; the measurement showed ordinary answers at that rate).
- Whether the consent text names Anthropic's clause word for word (proposal: yes).
- Whether a small crate for overwriting the token in memory (`zeroize`) is added; without it the program drops the token and cannot guarantee that the memory is cleared.

## 13. Risks and the way out

| Risk | Effect | Way out |
|---|---|---|
| Anthropic closes or changes the interface | state *not understood* or *refused*; last values stay, other sources continue | switch off; remove the Cargo feature in a later release; nothing else depends on it |
| Anthropic acts against the account | unknown (no case found) | the two locks; default off; consent text names the risk; the owner decides |
| Claude Code changes the credentials file | state *no sign-in* | parsing is tolerant; only the account source is lost |
| Token expires while Claude Code is not used | state *token expired*, last values stale | by design; no renewal in the cockpit |
| Rate limiting | state *rate limited*, longer interval | `Retry-After` and backoff are in the state machine |
