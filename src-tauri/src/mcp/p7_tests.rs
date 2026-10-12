use super::{contract::*, security::*};
use crate::database::Database;
use serde_json::{json, Value};
fn fixture() -> (Security, Database, Credentials) {
    let root = std::env::temp_dir().join(format!(
        "inline-p7-observed-{}",
        super::platform::random_secret().unwrap()
    ));
    let s = Security::open_at(root.join("private")).unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    let db = Database::open_at(root.join("data/inline.db")).unwrap();
    s.bind_diagnostic_database(db.clone());
    let c = s
        .grant(Grant {
            name: "synthetic".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
        .unwrap();
    (
        s,
        db,
        Credentials {
            client_id: c.client_id,
            token: c.token,
        },
    )
}
fn error(s: &Security, c: &Credentials, tool: &str) -> String {
    let id = super::platform::random_secret().unwrap();
    s.observe(
        c,
        tool,
        id.clone(),
        &Err(McpError::new("resync_required")),
        1,
        &Value::Null,
    );
    id
}
#[test]
fn diagnostics_own_only_pause_revoke_and_unhealthy_basis_are_enforced() {
    let (s, db, c) = fixture();
    let id = error(&s, &c, "query_tasks");
    let value =
        super::service::execute(&s, &db, &c, "get_diagnostic", json!({"errorId":id})).unwrap();
    assert_eq!(value["errorId"], id);
    assert!(!value.to_string().contains(&c.token));
    let other = s
        .grant(Grant {
            name: "other".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
        .unwrap();
    let other = Credentials {
        client_id: other.client_id,
        token: other.token,
    };
    assert_eq!(
        super::service::execute(&s, &db, &other, "get_diagnostic", json!({"errorId":id}))
            .unwrap_err()
            .code,
        "not_found"
    );
    s.pause(true).unwrap();
    assert_eq!(
        super::service::execute(&s, &db, &c, "get_diagnostic", json!({"errorId":id}))
            .unwrap_err()
            .code,
        "paused"
    );
    s.pause(false).unwrap();
    s.revoke(&c.client_id).unwrap();
    assert_eq!(
        super::service::execute(&s, &db, &c, "get_diagnostic", json!({"errorId":id}))
            .unwrap_err()
            .code,
        "revoked"
    );
}
#[test]
fn forged_client_never_claims_recent_state_and_post_guard_error_keeps_audit_reference() {
    let (s, _, c) = fixture();
    let fake = Credentials {
        client_id: c.client_id.clone(),
        token: "b".repeat(64),
    };
    let id = error(&s, &fake, "mutate_task");
    assert_eq!(
        s.diagnostics.own(&c.client_id, &id).unwrap_err().code,
        "not_found"
    );
    assert!(s
        .diagnostics
        .local("all", chrono::Utc::now().timestamp())
        .unwrap()["latestClients"]
        .as_array()
        .unwrap()
        .is_empty());
    let id = super::platform::random_secret().unwrap();
    s.observe(
        &c,
        "mutate_task",
        id.clone(),
        &Err(McpError::new("authorization_changed")),
        1,
        &json!({"auditId":77,"reason":"raw-business-canary","token":c.token}),
    );
    let v = s.diagnostics.own(&c.client_id, &id).unwrap();
    assert_eq!(v["auditId"], 77);
    assert!(!v.to_string().contains("raw-business-canary"));
}
#[test]
fn exact_preview_cancel_expiry_readback_and_no_overwrite() {
    let (s, db, c) = fixture();
    error(&s, &c, "query_tasks");
    let dir = std::env::temp_dir().join(format!(
        "inline-p7-export-{}",
        super::platform::random_secret().unwrap()
    ));
    std::fs::create_dir(&dir).unwrap();
    for format in ["json", "csv"] {
        let p = s
            .previews
            .preview(&s, &db, "faults", "export", format, "all", 100)
            .unwrap();
        let nonce = p["nonce"].as_str().unwrap();
        let path = dir.join(format!("summary.{format}"));
        let result = s
            .previews
            .confirm(&s, &db, nonce, path.to_str(), 100)
            .unwrap();
        assert_eq!(result["readbackMatchesPreview"], true);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            p["content"].as_str().unwrap()
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            if format == "json" { 1 } else { 2 }
        );
        assert!(!p["content"].as_str().unwrap().contains(&c.token));
        let p = s
            .previews
            .preview(&s, &db, "faults", "export", format, "all", 100)
            .unwrap();
        assert_eq!(
            s.previews
                .confirm(&s, &db, p["nonce"].as_str().unwrap(), path.to_str(), 100)
                .unwrap_err()
                .code,
            "conflict"
        );
        s.previews.cancel(p["nonce"].as_str().unwrap());
    }
    let p = s
        .previews
        .preview(&s, &db, "faults", "export", "json", "all", 100)
        .unwrap();
    let id = p["nonce"].as_str().unwrap();
    assert_eq!(
        s.previews.confirm(&s, &db, id, None, 700).unwrap_err().code,
        "snapshot_expired"
    );
    let p = s
        .previews
        .preview(&s, &db, "faults", "export", "json", "all", 800)
        .unwrap();
    let id = p["nonce"].as_str().unwrap();
    s.previews.cancel(id);
    assert_eq!(
        s.previews.confirm(&s, &db, id, None, 800).unwrap_err().code,
        "not_found"
    );
    assert_eq!(
        s.previews
            .preview(&s, &db, "faults", "clear", "json", "all", 100)
            .unwrap_err()
            .code,
        "invalid_arguments"
    );
}
#[test]
fn sensitive_notice_ack_is_persistent_and_does_not_grant_permission() {
    let (s, _, c) = fixture();
    let p = Permissions {
        regular_read: true,
        full_read: true,
        write: true,
    };
    assert_eq!(
        s.require_sensitive_notice(&p).unwrap_err().code,
        "sensitive_notice_required"
    );
    let rev = s.view().unwrap().revision;
    s.acknowledge_sensitive_notice().unwrap();
    s.require_sensitive_notice(&p).unwrap();
    let store = Security::read_at(s.private_root()).unwrap();
    assert!(store.sensitive_notice_acknowledged);
    let auth = store.authorize(&c).unwrap();
    assert!(!auth.permissions.write && !auth.permissions.full_read);
    assert!(auth.revision > rev);
}
#[test]
fn context_restores_and_read_success_does_not_resolve_unrelated_write_fault() {
    let (s, db, c) = fixture();
    assert!(super::observation::current().is_none());
    super::observation::with_request("a".repeat(64), || {
        assert_eq!(super::observation::current().unwrap(), "a".repeat(64));
    });
    assert!(super::observation::current().is_none());
    let id = error(&s, &c, "mutate_task");
    let caps = super::service::execute(&s, &db, &c, "get_capabilities", json!({}));
    s.observe(
        &c,
        "get_capabilities",
        super::platform::random_secret().unwrap(),
        &caps,
        1,
        &Value::Null,
    );
    let faults = s
        .diagnostics
        .local("all", chrono::Utc::now().timestamp())
        .unwrap();
    assert_eq!(faults["items"][0]["episodes"][0]["lastErrorId"], id);
    assert_eq!(faults["items"][0]["episodes"][0]["state"], "unresolved");
    assert_eq!(caps.unwrap()["tools"].as_array().unwrap().len(), 16);
}

#[test]
fn rejected_fault_identifier_expiry_and_private_export_destination_are_bounded() {
    let (s, db, c) = fixture();
    let id = error(&s, &c, "query_tasks");
    let now = chrono::Utc::now().timestamp();
    assert_eq!(
        s.diagnostics
            .own_at(&c.client_id, &id, now + 7 * 86400)
            .unwrap_err()
            .code,
        "not_found"
    );
    let p = s
        .previews
        .preview(&s, &db, "faults", "export", "json", "all", now)
        .unwrap();
    let private = s.private_root().join("leak.json");
    assert_eq!(
        s.previews
            .confirm(&s, &db, p["nonce"].as_str().unwrap(), private.to_str(), now)
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert!(!private.exists());
    let fake = "x".repeat(64);
    assert_eq!(
        s.diagnostics.own(&c.client_id, &fake).unwrap_err().code,
        "not_found"
    );
}
