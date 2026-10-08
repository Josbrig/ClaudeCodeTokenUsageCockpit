// SPDX-License-Identifier: Apache-2.0
//! Makes the short git commit available to the program as `GIT_COMMIT` (`unknown` if git is
//! not available, for example when building from a source archive).

use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=GIT_COMMIT={commit}");
    // Every commit appends to the log of HEAD, so the commit shown is never stale.
    println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
