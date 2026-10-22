//! Build-time readback only. A dirty local build never claims a future commit.
use serde_json::{json, Value};
/// Explicit offline packaging probe; no GUI, store, credentials or IPC startup.
pub fn print_if_requested(name: &str) -> bool {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 || args[0] != "--build-info" {
        return false;
    }
    use std::io::Write;
    let result = super::contract::Envelope::result(
        "offline-build-info".into(),
        Ok(json!({"name":name,"build":current(),"offline":true})),
    );
    let mut stdout = std::io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &result).is_err() || writeln!(stdout).is_err() {
        std::process::exit(6);
    }
    true
}
pub fn current() -> Value {
    identity(
        option_env!("IN_LINE_BUILD_BASELINE"),
        option_env!("IN_LINE_BUILD_VERIFIED") == Some("true"),
        option_env!("IN_LINE_BUILD_DIRTY") == Some("true"),
    )
}
pub fn compatible(peer: &Value) -> bool {
    let own = current();
    [
        "softwareVersion",
        "mcpVersion",
        "databaseSchemaVersion",
        "baselineCommit",
        "exactCommit",
        "sourceDirty",
        "commitVerification",
    ]
    .iter()
    .all(|key| peer.get(key).is_some_and(|v| Some(v) == own.get(key)))
}
fn identity(baseline: Option<&str>, verified: bool, dirty: bool) -> Value {
    let head = baseline.filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()));
    json!({"softwareVersion":env!("CARGO_PKG_VERSION"),"mcpVersion":super::contract::API_VERSION,"databaseSchemaVersion":crate::database::DATABASE_SCHEMA_VERSION,"baselineCommit":head,"exactCommit":if verified && !dirty{head}else{None},"sourceDirty":dirty,"commitVerification":if verified && !dirty && head.is_some(){"clean_checkout_readback"}else{"unverified_local_source"}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dirty_missing_or_checkout_mismatch_never_claims_exact_revision() {
        let sha = "1".repeat(40);
        for (head, verified, dirty) in [
            (Some(sha.as_str()), true, true),
            (Some(sha.as_str()), false, false),
            (None, true, false),
            (Some("unknown"), true, false),
        ] {
            assert!(identity(head, verified, dirty)["exactCommit"].is_null());
        }
        assert_eq!(identity(Some(&sha), true, false)["exactCommit"], sha);
    }
    #[test]
    fn mixed_builds_are_rejected_even_when_software_version_matches() {
        let own = current();
        assert!(compatible(&own));
        assert!(!compatible(&Value::Null));
        for (key, value) in [
            ("baselineCommit", json!("0".repeat(40))),
            ("databaseSchemaVersion", json!(0)),
            ("mcpVersion", json!(0)),
            ("sourceDirty", json!(!own["sourceDirty"].as_bool().unwrap())),
            ("exactCommit", json!("1".repeat(40))),
        ] {
            let mut peer = own.clone();
            peer[key] = value;
            assert!(!compatible(&peer), "{key}");
        }
    }
}
