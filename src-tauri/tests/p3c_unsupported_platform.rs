//! A Windows-only stdio crate cannot enforce this on a non-Windows CI target.
#![cfg(not(windows))]
#[test]
fn p3c_non_windows_never_accepts_github_actions() {
    use std::io::Write;
    assert!(
        std::env::var_os("GITHUB_ACTIONS").is_none(),
        "P3-C requires a GitHub-hosted Windows target; non-Windows CI is unsupported"
    );
    writeln!(std::io::stdout().lock(), "P3C_NOT_RUN {{\"fixture\":\"S-additions\",\"businessExecuted\":false,\"reason\":\"non_github_non_windows_environment\"}}").unwrap();
}
