use super::{contract::*, platform, security::Security};
use hmac::{Hmac, Mac};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};

const MAX_REQUEST: usize = 64 * 1024;
const MAX_RESPONSE: usize = 8 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(20);
#[derive(Serialize, Deserialize)]
struct Hello {
    version: u32,
    nonce: String,
}
#[derive(Serialize, Deserialize)]
struct Challenge {
    nonce: String,
    proof: String,
}
#[derive(Serialize, Deserialize)]
struct Request {
    credentials: Credentials,
    tool: String,
    args: Value,
}
#[derive(Serialize, Deserialize)]
struct Signed {
    payload: String,
    proof: String,
}
fn proof(secret: &str, bytes: &[u8]) -> Result<String, McpError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| McpError::new("security_unavailable"))?;
    mac.update(bytes);
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
async fn send<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    data: &T,
    max: usize,
) -> Result<(), McpError> {
    let bytes = serde_json::to_vec(data).map_err(|_| McpError::new("internal_error"))?;
    if bytes.len() > max {
        return Err(McpError::new("resource_limit"));
    }
    stream
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| McpError::new("host_unavailable"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|_| McpError::new("host_unavailable"))?;
    Ok(())
}
async fn receive<T: DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
    max: usize,
) -> Result<T, McpError> {
    let length = stream
        .read_u32()
        .await
        .map_err(|_| McpError::new("host_unavailable"))? as usize;
    if length > max {
        return Err(McpError::new("resource_limit"));
    }
    let mut data = vec![0; length];
    stream
        .read_exact(&mut data)
        .await
        .map_err(|_| McpError::new("host_unavailable"))?;
    serde_json::from_slice(&data).map_err(|_| McpError::new("invalid_arguments"))
}
fn listener(name: &str, first: bool) -> Result<NamedPipeServer, McpError> {
    let descriptor = platform::Descriptor::for_pipe()?;
    let mut attributes = descriptor.attributes();
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(17)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut windows_sys::Win32::Security::SECURITY_ATTRIBUTES).cast(),
            )
    }
    .map_err(|_| McpError::new("host_unavailable"))
}
pub type Dispatch = Arc<dyn Fn(&Credentials, &str, Value) -> Result<Value, McpError> + Send + Sync>;
pub fn start(security: Arc<Security>, dispatch: Dispatch) -> Result<(), McpError> {
    let name = security.pipe_name()?;
    // Create the first instance synchronously: startup fails closed on pipe squatting.
    // Tauri setup runs outside Tokio; register the IO handle with the same runtime that serves it.
    let runtime = tauri::async_runtime::handle();
    let _context = runtime.inner().enter();
    let first = listener(&name, true)?;
    tauri::async_runtime::spawn(serve(first, name, security, dispatch));
    Ok(())
}
pub async fn serve(
    mut current: NamedPipeServer,
    name: String,
    security: Arc<Security>,
    dispatch: Dispatch,
) {
    let permits = Arc::new(tokio::sync::Semaphore::new(15));
    loop {
        if current.connect().await.is_err() {
            break;
        }
        // Reserve next instance before releasing this handle, preventing endpoint takeover.
        let next = match listener(&name, false) {
            Ok(value) => value,
            Err(_) => break,
        };
        let mut connected = current;
        current = next;
        let permit = match permits.clone().try_acquire_owned() {
            Ok(value) => value,
            Err(_) => {
                continue;
            }
        };
        let security = security.clone();
        let dispatch = dispatch.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _ = tokio::time::timeout(DEADLINE, async {
                let hello: Hello = receive(&mut connected, 256).await?;
                if hello.version != API_VERSION || hello.nonce.len() != 64 {
                    return Err(McpError::new("incompatible"));
                }
                let secret = security
                    .store
                    .lock()
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .ipc_secret
                    .clone();
                let nonce = platform::random_secret()?;
                let challenge = Challenge {
                    proof: proof(
                        &secret,
                        format!("server:{}:{nonce}", hello.nonce).as_bytes(),
                    )?,
                    nonce: nonce.clone(),
                };
                send(&mut connected, &challenge, 512).await?;
                let signed: Signed = receive(&mut connected, MAX_REQUEST).await?;
                if !super::security::constant_equal(
                    &signed.proof,
                    &proof(
                        &secret,
                        format!("request:{nonce}:{}", signed.payload).as_bytes(),
                    )?,
                ) {
                    return Err(McpError::new("unauthenticated"));
                }
                let request: Request = serde_json::from_str(&signed.payload)
                    .map_err(|_| McpError::new("invalid_arguments"))?;
                let caller = request.credentials.clone();
                let revision = security
                    .store
                    .lock()
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .revision;
                let result = tokio::task::spawn_blocking(move || {
                    let _work_permit = _permit;
                    dispatch(&request.credentials, &request.tool, request.args)
                })
                .await
                .map_err(|_| McpError::new("internal_error"))?;
                // Do not release a materialized result under a superseded authorization epoch.
                let result = super::service::recheck_result(&security, &caller, revision, result);
                let payload =
                    serde_json::to_string(&result).map_err(|_| McpError::new("internal_error"))?;
                let response = Signed {
                    proof: proof(
                        &secret,
                        format!("response:{}:{payload}", hello.nonce).as_bytes(),
                    )?,
                    payload,
                };
                send(&mut connected, &response, MAX_RESPONSE).await
            })
            .await;
        });
    }
}

pub async fn call(credentials: Credentials, tool: &str, args: Value) -> Result<Value, McpError> {
    let root = platform::root()?;
    call_at(root, credentials, tool, args).await
}
pub async fn call_at(
    root: std::path::PathBuf,
    credentials: Credentials,
    tool: &str,
    args: Value,
) -> Result<Value, McpError> {
    fn local_limit(credentials: &Credentials, valid: bool) -> Result<(), McpError> {
        use super::security::Limiter;
        use std::sync::{Mutex, OnceLock};
        static LIMITS: OnceLock<Mutex<Limiter>> = OnceLock::new();
        LIMITS
            .get_or_init(|| Mutex::new(Limiter::default()))
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?
            .check(
                Limiter::key(credentials, valid),
                valid,
                std::time::Instant::now(),
            )
    }
    if credentials.client_id.len() != 64 || credentials.token.len() != 64 {
        local_limit(&credentials, false)?;
        return Err(McpError::new("unauthenticated"));
    }
    let store = Security::read_at(&root)?;
    let authorization = store.authorize(&credentials);
    local_limit(&credentials, authorization.is_ok())?;
    authorization?; // Never launch/migrate a host for invalid or paused credentials.
    let name = {
        use sha2::Digest;
        format!(
            r"\\.\pipe\in-line-mcp-{}",
            sha2::Sha256::digest(
                root.join("authorization.dpapi")
                    .to_string_lossy()
                    .as_bytes()
            )
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
        )
    };
    tokio::time::timeout(DEADLINE, async {
        let mut spawned = false;
        let mut stream = loop {
            match ClientOptions::new().open(&name) {
                Ok(pipe) => break pipe,
                Err(error) if error.raw_os_error() == Some(2) => {
                    if !spawned {
                        launch_host()?;
                        spawned = true;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) if error.raw_os_error() == Some(231) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(_) => return Err(McpError::new("host_unavailable")),
            }
        };
        use std::os::windows::io::AsRawHandle;
        platform::verify_server(stream.as_raw_handle())?;
        let nonce = platform::random_secret()?;
        send(
            &mut stream,
            &Hello {
                version: API_VERSION,
                nonce: nonce.clone(),
            },
            256,
        )
        .await?;
        let challenge: Challenge = receive(&mut stream, 512).await?;
        if challenge.nonce.len() != 64
            || !super::security::constant_equal(
                &challenge.proof,
                &proof(
                    &store.ipc_secret,
                    format!("server:{nonce}:{}", challenge.nonce).as_bytes(),
                )?,
            )
        {
            return Err(McpError::new("host_unavailable"));
        }
        // Credentials are sent only after authenticated server challenge proof.
        let payload = serde_json::to_string(&Request {
            credentials,
            tool: tool.into(),
            args,
        })
        .map_err(|_| McpError::new("invalid_arguments"))?;
        let signed = Signed {
            proof: proof(
                &store.ipc_secret,
                format!("request:{}:{payload}", challenge.nonce).as_bytes(),
            )?,
            payload,
        };
        send(&mut stream, &signed, MAX_REQUEST).await?;
        let response: Signed = receive(&mut stream, MAX_RESPONSE).await?;
        if !super::security::constant_equal(
            &response.proof,
            &proof(
                &store.ipc_secret,
                format!("response:{nonce}:{}", response.payload).as_bytes(),
            )?,
        ) {
            return Err(McpError::new("host_unavailable"));
        }
        serde_json::from_str(&response.payload).map_err(|_| McpError::new("host_unavailable"))?
    })
    .await
    .map_err(|_| McpError::new("host_unavailable"))?
}
fn launch_host() -> Result<(), McpError> {
    use std::os::windows::process::CommandExt;
    // An older running host may forward --mcp-background by showing its UI. Fail closed.
    if platform::host_already_running() {
        return Err(McpError::new("host_unavailable"));
    }
    let executable = std::env::current_exe()
        .map_err(|_| McpError::new("host_unavailable"))?
        .with_file_name("in-line.exe");
    let mut command = std::process::Command::new(executable);
    command
        .arg("--mcp-background")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000);
    command
        .env_remove("IN_LINE_MCP_CLIENT_ID")
        .env_remove("IN_LINE_MCP_TOKEN");
    command
        .spawn()
        .map_err(|_| McpError::new("host_unavailable"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn impostor_pipe_cannot_receive_credentials_or_replace_first_instance() {
        let root = std::env::temp_dir().join(format!(
            "inline-mcp-impostor-{}",
            platform::random_secret().unwrap()
        ));
        let security = Security::open_at(root.clone()).unwrap();
        let issued = security
            .grant(super::super::security::Grant {
                name: "隔离伪造端点测试".into(),
                permissions: Default::default(),
                scope: Default::default(),
            })
            .unwrap();
        let name = security.pipe_name().unwrap();
        let mut impostor = listener(&name, true).unwrap();
        assert!(listener(&name, true).is_err());
        let creds = Credentials {
            client_id: issued.client_id,
            token: issued.token,
        };
        // call_at authenticates its independent store, then rejects this test binary's path
        // before sending even the handshake, much less a credential.
        let call = tokio::spawn(call_at(
            root,
            creds,
            "get_capabilities",
            serde_json::json!({}),
        ));
        impostor.connect().await.unwrap();
        assert_eq!(call.await.unwrap().unwrap_err().code, "host_unavailable");
        let mut byte = [0u8];
        let read = impostor.read(&mut byte).await;
        assert!(matches!(read, Ok(0)) || read.is_err());
    }
    #[tokio::test]
    async fn framing_rejects_oversized_request_before_allocation() {
        let (mut a, mut b) = tokio::io::duplex(8);
        a.write_u32((MAX_REQUEST + 1) as u32).await.unwrap();
        assert_eq!(
            receive::<Value>(&mut b, MAX_REQUEST)
                .await
                .unwrap_err()
                .code,
            "resource_limit"
        );
    }
    #[test]
    fn proofs_bind_direction_and_nonce() {
        let s = "test";
        assert_ne!(
            proof(s, b"server:one").unwrap(),
            proof(s, b"request:one").unwrap()
        );
        assert_ne!(
            proof(s, b"server:one").unwrap(),
            proof(s, b"server:two").unwrap()
        );
    }
}
