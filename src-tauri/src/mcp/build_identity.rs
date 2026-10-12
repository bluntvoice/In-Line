//! Build-time readback only. A dirty local build never claims a future commit.
use serde_json::{json, Value};
pub fn current() -> Value {
    identity(
        option_env!("IN_LINE_BUILD_BASELINE"),
        option_env!("IN_LINE_BUILD_VERIFIED") == Some("true"),
        option_env!("IN_LINE_BUILD_DIRTY") == Some("true"),
    )
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
}
