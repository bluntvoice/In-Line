#![cfg(windows)]
//! Real GUI host autostart, isolated storage and app identifier, no real business database.
use in_line_lib::{
    database::Database,
    mcp::{
        contract::Credentials,
        platform,
        security::{Grant, Permissions, Scope, Security},
    },
};
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::json;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
};
struct Cleanup {
    pid: u32,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        unsafe {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE},
            };
            let process = OpenProcess(PROCESS_TERMINATE, 0, self.pid);
            if !process.is_null() {
                TerminateProcess(process, 0);
                CloseHandle(process);
            }
        }
    }
}
unsafe extern "system" fn visible(hwnd: HWND, param: LPARAM) -> i32 {
    let (pid, count) = &mut *(param as *mut (u32, usize));
    let mut owner = 0;
    GetWindowThreadProcessId(hwnd, &mut owner);
    if owner == *pid && IsWindowVisible(hwnd) != 0 {
        *count += 1;
    }
    1
}
#[tokio::test]
async fn real_host_background_single_instance_and_protocol(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("inline-mcp-host-{}", platform::random_secret()?));
    std::fs::create_dir_all(&root)?;
    let data = root.join("data");
    std::fs::create_dir_all(&data)?;
    let db = Database::open_at(data.join("inline.db")).map_err(std::io::Error::other)?;
    drop(db);
    let security_root = root.join("security");
    let security = Security::open_at(security_root.clone())?;
    let issued = security.grant(Grant {
        name: "主程序集成测试".into(),
        permissions: Permissions::default(),
        scope: Scope::default(),
    })?;
    // Start stdio only: it must authenticate before it silently starts the actual main binary.
    let transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
            command
                .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
                .env("IN_LINE_MCP_TEST_DATA_ROOT", &data)
                .env("IN_LINE_MCP_CLIENT_ID", &issued.client_id)
                .env("IN_LINE_MCP_TOKEN", &issued.token)
                .env_remove("IN_LINE_MCP_TEST_HOST_EXE");
        }),
    )?;
    let client = ().serve(transport).await?;
    let result = client
        .call_tool(
            CallToolRequestParams::new("get_capabilities")
                .with_arguments(serde_json::from_value(json!({}))?),
        )
        .await?;
    assert_eq!(
        result.structured_content.as_ref().unwrap()["status"],
        "ok",
        "{result:?}"
    );
    // Locate the exact isolated pipe owner instead of killing any existing installed In Line.
    let pipe = tokio::net::windows::named_pipe::ClientOptions::new().open(security.pipe_name()?)?;
    use std::os::windows::io::AsRawHandle;
    let mut pid = 0;
    unsafe {
        assert_ne!(
            windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId(
                pipe.as_raw_handle(),
                &mut pid
            ),
            0
        );
    }
    let _cleanup = Cleanup { pid };
    drop(pipe);
    let mut count = (pid, 0);
    unsafe {
        EnumWindows(Some(visible), &mut count as *mut _ as LPARAM);
    }
    assert_eq!(count.1, 0, "background host must have no visible windows");
    let mut foreground = 0;
    unsafe {
        GetWindowThreadProcessId(
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow(),
            &mut foreground,
        );
    }
    assert_ne!(foreground, pid, "background host must not take focus");
    let mut secondary = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line"));
    secondary
        .arg("--mcp-background")
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
        .env("IN_LINE_MCP_TEST_DATA_ROOT", &data)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    use std::os::windows::process::CommandExt;
    secondary.as_std_mut().creation_flags(0x08000000);
    let status =
        tokio::time::timeout(std::time::Duration::from_secs(15), secondary.status()).await??;
    assert!(status.success());
    count.1 = 0;
    unsafe {
        EnumWindows(Some(visible), &mut count as *mut _ as LPARAM);
    }
    assert_eq!(count.1, 0, "secondary background launch must stay hidden");
    unsafe {
        GetWindowThreadProcessId(
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow(),
            &mut foreground,
        );
    }
    assert_ne!(foreground, pid);
    let summary = client
        .call_tool(
            CallToolRequestParams::new("get_report_summary").with_arguments(
                serde_json::from_value(json!({"startDate":"2026-08-08","endDate":"2026-08-08"}))?,
            ),
        )
        .await?;
    assert_eq!(
        summary.structured_content.unwrap()["data"]["statistics"]["summary"]["handledTasks"],
        0
    );
    let stored = Security::read_at(&security_root)?;
    assert!(stored
        .authorize(&Credentials {
            client_id: issued.client_id,
            token: issued.token
        })
        .is_ok());
    client.cancel().await?;
    Ok(())
}
