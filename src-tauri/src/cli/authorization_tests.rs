use super::*;
use crate::mcp::{onboarding::tests::Fixture, security::Permissions};
fn exe(f: &Fixture) -> PathBuf {
    let exe = f.command.with_file_name("in-line-cli.exe");
    std::fs::write(&exe, b"isolated synthetic CLI").unwrap();
    exe
}
fn args(client: &str) -> PrepareArgs {
    PrepareArgs {
        client: client.into(),
        permissions: Permissions::default(),
        scope: Default::default(),
    }
}
#[test]
fn gui_issues_distinct_cli_profiles_without_credentials_in_prompt_or_mcp_reuse() {
    let f = Fixture::new();
    let exe = exe(&f);
    let root = f.security_root();
    let mcp = f.prepare();
    let a = prepare_at(&f.security, &root, &exe, args("codex-cli"), 1000).unwrap();
    let b = prepare_at(&f.security, &root, &exe, args("powershell-cli"), 1000).unwrap();
    let (ta, _) = load(&root, &a.package_id, &exe, 1000).unwrap();
    let (tb, _) = load(&root, &b.package_id, &exe, 1000).unwrap();
    assert_ne!(a.client_id, b.client_id);
    assert_ne!(mcp.client_id, a.client_id);
    assert_ne!(ta.profile.credentials.token, tb.profile.credentials.token);
    assert_ne!(ta.profile.name, tb.profile.name);
    for (receipt, ticket) in [(&a, &ta), (&b, &tb)] {
        let text = serde_json::to_string(receipt).unwrap();
        assert!(!text.contains(&ticket.profile.credentials.token));
        assert!(receipt.prompt.contains("client import --package"));
        assert!(receipt.prompt.contains("space '' & client"));
        assert!(receipt.prompt.contains("--profile cli-"));
        assert!(receipt.prompt.contains("不得以CLI验证冒充"));
    }
    assert_eq!(
        load(&root, &a.package_id, &exe, 1600).err().unwrap().code,
        "import_expired"
    );
    assert!(load(&root, &mcp.package_id, &exe, 1000).is_err());
    let count = f.security.view().unwrap().clients.len();
    let mut write = args("codex-cli");
    write.permissions.write = true;
    assert!(prepare_at(&f.security, &root, &exe, write, 1000).is_err());
    assert!(prepare_at(&f.security, &root, &exe, args("unknown"), 1000).is_err());
    assert_eq!(count, f.security.view().unwrap().clients.len());
}
#[test]
fn sealed_package_binding_single_use_and_rotation_keep_failures_local() {
    let f = Fixture::new();
    let exe = exe(&f);
    let root = f.security_root();
    let a = prepare_at(&f.security, &root, &exe, args("codex-cli"), 1000).unwrap();
    let (ticket, dir) = load(&root, &a.package_id, &exe, 1000).unwrap();
    let original = onboarding::read_private(&dir.join("ticket.dpapi")).unwrap();
    for field in ["purpose", "selectedClient", "version", "expiresAt", "extra"] {
        let mut v = serde_json::to_value(&ticket).unwrap();
        v[field] = json!("invalid");
        let bytes = platform::seal(&serde_json::to_vec(&v).unwrap(), false).unwrap();
        std::fs::write(dir.join("ticket.dpapi"), &bytes).unwrap();
        assert!(load(&root, &a.package_id, &exe, 1000).is_err());
        assert_eq!(std::fs::read(dir.join("ticket.dpapi")).unwrap(), bytes);
    }
    for field in [
        "machine",
        "sid",
        "purpose",
        "binaryHash",
        "name",
        "executable",
        "softwareVersion",
        "extra",
    ] {
        let mut v = serde_json::to_value(&ticket).unwrap();
        v["profile"][field] = json!("invalid");
        std::fs::write(
            dir.join("ticket.dpapi"),
            platform::seal(&serde_json::to_vec(&v).unwrap(), false).unwrap(),
        )
        .unwrap();
        assert!(load(&root, &a.package_id, &exe, 1000).is_err());
    }
    std::fs::write(dir.join("ticket.dpapi"), original).unwrap();
    onboarding::write_private(&dir.join("attempt.lock"), b"").unwrap();
    assert_eq!(
        load(&root, &a.package_id, &exe, 1000).err().unwrap().code,
        "import_in_doubt"
    );
    std::fs::remove_file(dir.join("attempt.lock")).unwrap();
    let profile_path = root
        .join("cli-profiles")
        .join(format!("{}.dpapi", ticket.profile.name));
    let sealed = platform::seal(&serde_json::to_vec(&ticket.profile).unwrap(), false).unwrap();
    onboarding::write_private(&profile_path, &sealed).unwrap();
    let b = rotate_at(&f.security, &root, &exe, &a.client_id, 1001).unwrap();
    assert!(load(&root, &a.package_id, &exe, 1001).is_err());
    let (next, _) = load(&root, &b.package_id, &exe, 1001).unwrap();
    assert_eq!(next.previous_hash, Some(hash(&sealed)));
    assert_eq!(next.profile.name, ticket.profile.name);
    assert_ne!(
        next.profile.credentials.token,
        ticket.profile.credentials.token
    );
}
