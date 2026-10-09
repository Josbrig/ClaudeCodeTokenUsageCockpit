# usage-probe

Test program for REQ-124 (account usage percentages with the stored token of Claude Code). It answers one question: **can the 5-hour and 7-day usage percentages of the own subscription be read with the sign-in token that Claude Code has stored on the same computer, and how does the interface behave?**

It is a separate small project (own `Cargo.toml` and `Cargo.lock`, not a member of the main workspace), so it is not part of the build of `usage-cockpit`.

## What it does

1. Reads the credentials file of Claude Code (`%USERPROFILE%\.claude\.credentials.json`, or the folder named in `CLAUDE_CONFIG_DIR`) into memory and takes the access token and its expiry from it.
2. Asks the interface `GET https://api.anthropic.com/api/oauth/usage` (headers `Authorization: Bearer ...` and `anthropic-beta: oauth-2025-04-20`) a few times, by default **5 times, 60 seconds apart**. At most 20 calls and never faster than every 15 seconds.
3. Prints one line per call: time (UTC), HTTP status, duration, the percentage and the reset time of both windows, and how long the token is still valid.
4. Stops at the first answer that is not 200, and does not ask again after a rate limit (429).
5. At the end checks that the credentials file has exactly the bytes it had before.

What it never does: print, write, copy or refresh the token (a line that contains the token is not printed, whatever the server answers), send anything to a destination other than the interface above, or change any file.

## Run

```
cd tools/usage-probe
cargo run --release                          # 5 calls, 60 seconds apart
cargo run --release -- --calls 3 --interval 120
cargo test                                   # the unit tests (no network)
```

Exit code 0: all calls answered 200 and the credentials file is unchanged; 1: a call failed or the file changed; 2: wrong arguments or no credentials.

## What the interface is

The interface is not documented by Anthropic. It returns the same figures as `/usage` of Claude Code: for each window `utilization` (percent used) and `resets_at` (ISO 8601, UTC), plus further fields that change and are ignored here. The token must have the scope `user:profile`, which the token that Claude Code stores has. The access token is valid for about 8 hours and is renewed by Claude Code when it is used; this program never renews it.

## Terms of use

Reading the interface with the token is an access to the Services by a program. The Consumer Terms of Service list as a prohibited use: "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it, to access the Services through automated or non-human means, whether through a bot, script, or otherwise." No explicit permission was found and no explicit ban of this case either; how Anthropic applies the clause is open, and the interface can change or be closed at any time. Details and sources: [../../docs/research/token-dimensions-and-terms.md](../../docs/research/token-dimensions-and-terms.md). Use the program only on your own account and knowingly.
