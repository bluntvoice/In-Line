use crate::mcp::contract::Envelope;
use std::io::Write;

pub fn exit_code(envelope: &Envelope) -> i32 {
    if envelope.status == "ok" && envelope.error.is_none() && envelope.data.is_some() {
        return 0;
    }
    match envelope
        .error
        .as_ref()
        .and_then(|e| e["code"].as_str())
        .unwrap_or("internal_error")
    {
        "invalid_arguments" | "unsupported" => 2,
        "unauthenticated"
        | "forbidden"
        | "revoked"
        | "paused"
        | "explicit_intent_required"
        | "sensitive_notice_required" => 3,
        "not_found" => 4,
        "conflict"
        | "idempotency_conflict"
        | "business_rule"
        | "possible_duplicate"
        | "needs_disambiguation" => 5,
        "host_unavailable"
        | "security_unavailable"
        | "incompatible"
        | "rate_limited"
        | "snapshot_expired"
        | "cursor_invalid"
        | "snapshot_invalid"
        | "resync_required"
        | "result_expired"
        | "pause_expired"
        | "recovery_expired"
        | "clock_invalid"
        | "resource_limit"
        | "waiter_cancelled" => 6,
        "job_not_complete" => 8,
        "result_unknown" | "authorization_changed" => 9,
        _ => 7,
    }
}
pub(super) fn emit(envelope: &Envelope, json_output: bool) -> i32 {
    let code = exit_code(envelope);
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let result = if json_output {
        serde_json::to_writer(&mut stdout, envelope)
            .map_err(std::io::Error::other)
            .and_then(|_| writeln!(stdout))
    } else if code != 0 {
        writeln!(
            stderr,
            "{}",
            serde_json::to_string(envelope).unwrap_or_default()
        )
    } else if let Some(text) = envelope.data.as_ref().and_then(|d| d["help"].as_str()) {
        writeln!(stdout, "{text}")
    } else if let Some(data) = envelope.data.as_ref().and_then(|d| d.as_object()) {
        writeln!(stdout, "字段\t值").and_then(|_| {
            data.iter()
                .try_for_each(|(key, value)| writeln!(stdout, "{key}\t{value}"))
        })
    } else {
        Err(std::io::Error::other("invalid local envelope"))
    };
    if result.is_err() {
        6
    } else {
        code
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::contract::McpError;
    #[test]
    fn error_categories_preserve_unknown_results_and_do_not_default_to_success() {
        for (code, expected) in [
            ("invalid_arguments", 2),
            ("unauthenticated", 3),
            ("revoked", 3),
            ("not_found", 4),
            ("business_rule", 5),
            ("host_unavailable", 6),
            ("incompatible", 6),
            ("job_not_complete", 8),
            ("result_unknown", 9),
            ("authorization_changed", 9),
            ("unrecognized", 7),
        ] {
            assert_eq!(
                exit_code(&Envelope::result("test".into(), Err(McpError::new(code)))),
                expected
            );
        }
        assert_eq!(
            exit_code(&Envelope::result("test".into(), Ok(serde_json::json!({})))),
            0
        );
        let mut malformed = Envelope::result("test".into(), Ok(serde_json::json!({})));
        malformed.status = "unknown".into();
        assert_eq!(exit_code(&malformed), 7);
    }
}
