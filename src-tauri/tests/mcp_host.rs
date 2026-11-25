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
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    io::{self, Read, Write},
    mem::size_of_val,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, HWND, LPARAM},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW},
        Threading::CREATE_NO_WINDOW,
    },
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
};

const TEST: &str = "real_host_background_single_instance_and_protocol";
const WORKER_ROOT: &str = "IN_LINE_MCP_HOST_TEST_WORKER_ROOT";
const WORKER_LIMIT: Duration = Duration::from_secs(120);
const CLEANUP_LIMIT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

fn stage<T, E>(label: &str, action: impl FnOnce() -> Result<T, E>) -> io::Result<T> {
    eprintln!("mcp_host: {label}: begin");
    let start = Instant::now();
    let value = action().map_err(|_| io::Error::other(format!("{label} failed")))?;
    eprintln!("mcp_host: {label}: ok ({} ms)", start.elapsed().as_millis());
    Ok(value)
}

async fn timed<T, E>(
    label: &str,
    seconds: u64,
    action: impl Future<Output = Result<T, E>>,
) -> io::Result<T> {
    eprintln!("mcp_host: {label}: begin ({seconds}s limit)");
    let start = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(seconds), action)
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{label} exceeded {seconds}s"),
            )
        })?
        .map_err(|_| io::Error::other(format!("{label} failed")))?;
    eprintln!("mcp_host: {label}: ok ({} ms)", start.elapsed().as_millis());
    Ok(result)
}

struct ProcessJob(OwnedHandle);

impl ProcessJob {
    fn new() -> io::Result<Self> {
        // Null attributes mean a non-inheritable, unnamed handle.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // This process owns the new Job handle. Closing it kills any remaining members.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(handle) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = unsafe {
            SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast(),
                size_of_val(&limits) as u32,
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn active_processes(&self) -> io::Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let queried = unsafe {
            QueryInformationJobObject(
                self.0.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                ptr::from_mut(&mut info).cast(),
                size_of_val(&info) as u32,
                ptr::null_mut(),
            )
        };
        if queried == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info.ActiveProcesses)
    }

    fn stop(&self) -> io::Result<()> {
        if self.active_processes()? != 0
            && unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let deadline = Instant::now() + CLEANUP_LIMIT;
        while self.active_processes()? != 0 {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "test Job did not drain within 5s",
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        eprintln!("mcp_host: Job cleanup: active_processes=0");
        Ok(())
    }
}

fn wait_for_exit(child: &mut Child, deadline: Instant) -> io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "host test worker exceeded its deadline",
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

// Same namespace derivation as Security::pipe_name, with an input known to the supervisor.
fn check_pipe_absent(root: &Path) -> io::Result<()> {
    let key = root.join("security").join("authorization.dpapi");
    let digest = Sha256::digest(key.to_string_lossy().as_bytes());
    let name = format!(r"\\.\pipe\in-line-mcp-{digest:x}");
    let mut wide: Vec<u16> = std::ffi::OsStr::new(&name).encode_wide().collect();
    wide.push(0);
    if unsafe { WaitNamedPipeW(wide.as_ptr(), 1) } != 0 {
        return Err(io::Error::other("the test pipe still exists"));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_FILE_NOT_FOUND as i32) {
        return Err(error);
    }
    Ok(())
}

#[test]
fn real_host_background_single_instance_and_protocol() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(root) = std::env::var_os(WORKER_ROOT) {
        // The supervisor grants this byte only after a successful Job assignment.
        let mut release = [0];
        io::stdin().read_exact(&mut release)?;
        if release != [1] {
            return Err(io::Error::other("missing worker release").into());
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let status = runtime.block_on(run_host_checks(Path::new(&root)));
        eprintln!("mcp_host: worker runtime shutdown: begin");
        drop(runtime);
        return status;
    }

    let root = std::env::temp_dir().join(format!("inline-mcp-host-{}", platform::random_secret()?));
    stage("test pipe absent before worker launch", || {
        check_pipe_absent(&root)
    })?;
    let job = ProcessJob::new().map_err(|e| io::Error::other(format!("create test Job: {e}")))?;
    let deadline = Instant::now() + WORKER_LIMIT;
    let mut worker = Command::new(std::env::current_exe()?)
        .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
        .env(WORKER_ROOT, &root)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Err(error) = job.assign(&worker) {
        // EOF is not permission to run. Stop this original child handle on an assignment error.
        drop(worker.stdin.take());
        let kill = worker.kill();
        let exit = wait_for_exit(&mut worker, Instant::now() + CLEANUP_LIMIT);
        return Err(io::Error::other(format!(
            "unable to assign host worker to Job: {error}; worker kill={kill:?}, exit={exit:?}"
        ))
        .into());
    }

    let result = (|| {
        let mut release = worker
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing worker stdin"))?;
        release.write_all(&[1])?;
        drop(release);
        let status = wait_for_exit(&mut worker, deadline)?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "host test worker exited with {status}"
            )));
        }
        Ok(())
    })();

    // Always clean up, even if the worker panics, times out or fails before discovering the pipe.
    // Use Job membership, not a later lookup of a potentially reused PID.
    let cleanup = (|| {
        job.stop()?;
        wait_for_exit(&mut worker, Instant::now() + CLEANUP_LIMIT)?;
        stage("test pipe absent after Job cleanup", || {
            check_pipe_absent(&root)
        })
    })();
    if let Err(error) = &cleanup {
        eprintln!("mcp_host: cleanup failed: {error}");
    }
    // Keep only synthetic fixture files for the isolated executor to inspect.
    eprintln!("mcp_host: synthetic fixture left in the executor TEMP root");
    result?;
    cleanup?;
    Ok(())
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

async fn run_host_checks(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let data = root.join("data");
    stage("fixture directories", || {
        std::fs::create_dir(root)?;
        std::fs::create_dir(&data)
    })?;
    stage("Database::open_at and close", || {
        Database::open_at(data.join("inline.db")).map(drop)
    })?;
    let security_root = root.join("security");
    let security = stage("Security::open_at", || {
        Security::open_at(security_root.clone())
    })?;
    let issued = stage("security.grant", || {
        security.grant(Grant {
            name: "主程序集成测试".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
    })?;

    // Start stdio only: it must authenticate before it silently starts the actual main binary.
    let transport = stage("sidecar process spawn", || {
        TokioChildProcess::new(
            tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
                command
                    .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
                    .env("IN_LINE_MCP_TEST_DATA_ROOT", &data)
                    .env("IN_LINE_MCP_CLIENT_ID", &issued.client_id)
                    .env("IN_LINE_MCP_TOKEN", &issued.token)
                    .env_remove("IN_LINE_MCP_TEST_HOST_EXE")
                    .kill_on_drop(true);
            }),
        )
    })?;
    let client = timed("stdio initialize", 20, ().serve(transport)).await?;
    let result = timed(
        "get_capabilities",
        30,
        client.call_tool(
            CallToolRequestParams::new("get_capabilities")
                .with_arguments(serde_json::from_value(json!({}))?),
        ),
    )
    .await?;
    assert!(
        result.structured_content.as_ref().unwrap()["status"] == "ok",
        "get_capabilities must authorize the isolated client"
    );

    let name = stage("compute test pipe name", || security.pipe_name())?;
    let pipe = stage("open test pipe", || {
        tokio::net::windows::named_pipe::ClientOptions::new().open(&name)
    })?;
    eprintln!("mcp_host: resolve pipe owner: begin");
    let mut pid = 0;
    unsafe {
        assert_ne!(
            GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid),
            0
        );
    }
    drop(pipe);
    eprintln!("mcp_host: resolve pipe owner: ok");
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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    secondary.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    let mut secondary = stage("secondary host spawn", || secondary.spawn())?;
    let status = timed("secondary host exit", 15, secondary.wait()).await?;
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
    eprintln!("mcp_host: secondary host window and focus checks: ok");
    let summary = timed(
        "get_report_summary",
        30,
        client.call_tool(
            CallToolRequestParams::new("get_report_summary").with_arguments(
                serde_json::from_value(json!({"startDate":"2026-08-08","endDate":"2026-08-08"}))?,
            ),
        ),
    )
    .await?;
    assert!(
        summary.structured_content.unwrap()["data"]["statistics"]["summary"]["handledTasks"] == 0,
        "the isolated database must have no handled tasks"
    );
    let stored = stage("Security::read_at", || Security::read_at(&security_root))?;
    assert!(stored
        .authorize(&Credentials {
            client_id: issued.client_id,
            token: issued.token,
        })
        .is_ok());
    timed("client.cancel", 10, client.cancel()).await?;
    Ok(())
}
