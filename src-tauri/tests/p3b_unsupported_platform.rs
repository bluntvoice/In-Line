//! The Windows-only stdio test crate cannot reject a non-Windows CI host itself.
#![cfg(not(windows))]

#[test]
fn p3b_non_windows_never_accepts_github_actions() {
    use std::io::Write;
    assert!(
        std::env::var_os("GITHUB_ACTIONS").is_none(),
        "P3-B requires a GitHub-hosted Windows target; non-Windows CI is unsupported"
    );
    for fixture in ["Q", "J"] {
        writeln!(std::io::stdout().lock(), "P3B_NOT_RUN {{\"fixture\":\"{fixture}\",\"status\":\"not_run\",\"reason\":\"non_github_non_windows_environment\",\"businessExecuted\":false}}" ).unwrap();
    }
}
