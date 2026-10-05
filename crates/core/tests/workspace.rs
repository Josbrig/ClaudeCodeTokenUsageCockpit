// SPDX-License-Identifier: Apache-2.0

#[test]
fn req_110_workspace_builds() {
    // Exists so that CI has at least one test to run; real tests follow with the features.
    // Checks the package name so the test says something about the workspace layout.
    assert_eq!(env!("CARGO_PKG_NAME"), "cockpit-core");
}
