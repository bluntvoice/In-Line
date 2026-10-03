//! Optional, app-only fonts. Network access happens only after an explicit command.
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub const FAMILY: &str = "in-line:sarasa-ui-sc";
const EVENT: &str = "recommended-font-progress";
// Immutable commit already published on the development branch. Full official
// v1.0.42 fonts, losslessly converted to WOFF2; no release or latest URL involved.
const SOURCE: &str = "https://raw.githubusercontent.com/bluntvoice/In-Line/bc211d15f8d5d632c4dab0747eab57a8d91fb55c/public/fonts";
const VERSION_DIR: &str = "sarasa-ui-sc-1.0.42";
pub struct FontAsset<'a> {
    pub name: &'a str,
    pub size: u64,
    pub digest: &'a str,
}
pub const ASSETS: [FontAsset<'static>; 3] = [
    FontAsset {
        name: "SarasaUiSC-Regular.woff2",
        size: 8343180,
        digest: "d1af2cf8d2ae42657d5f199707bddb4ea08db6c05d24373c6acd542295148e0a",
    },
    FontAsset {
        name: "SarasaUiSC-SemiBold.woff2",
        size: 8479044,
        digest: "2735b9303427f98608119c84a363097a47cbb71d3f3adecf90f5a96edb3accb7",
    },
    FontAsset {
        name: "SarasaUiSC-Bold.woff2",
        size: 8621424,
        digest: "e1c39dd332beae687d06809e2816b1189f919f6eb6274baa7dccf3e39f959f58",
    },
];
fn total() -> u64 {
    ASSETS.iter().map(|asset| asset.size).sum()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: u8,
    pub message: Option<String>,
}
impl FontProgress {
    fn new(phase: &str, downloaded_bytes: u64, message: Option<String>) -> Self {
        Self {
            phase: phase.into(),
            downloaded_bytes,
            total_bytes: total(),
            percent: (downloaded_bytes.saturating_mul(100) / total()).min(100) as u8,
            message,
        }
    }
}
pub struct FontManager {
    busy: AtomicBool,
    selection_revision: Mutex<u64>,
    progress: Mutex<FontProgress>,
}
impl Default for FontManager {
    fn default() -> Self {
        Self {
            busy: AtomicBool::new(false),
            selection_revision: Mutex::new(0),
            progress: Mutex::new(FontProgress::new("idle", 0, None)),
        }
    }
}
impl FontManager {
    fn snapshot(&self) -> Result<FontProgress, String> {
        self.progress
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "无法读取字体下载状态".into())
    }
}
pub fn save_selection(
    app: &AppHandle,
    db: &crate::database::Database,
    value: String,
) -> Result<(), String> {
    let manager = app.state::<FontManager>();
    let mut revision = manager
        .selection_revision
        .lock()
        .map_err(|_| "字体设置不可用")?;
    db.set_setting("ui_font_family".into(), value)?;
    *revision += 1;
    Ok(())
}

pub fn root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| "无法访问应用数据目录")?
        .join("recommended-fonts")
        .join(VERSION_DIR))
}
fn reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn safe_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|value| value.is_dir() && !reparse(&value))
}
fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|value| value.is_file() && !reparse(&value))
}
fn verify_bytes(asset: &FontAsset, bytes: &[u8]) -> bool {
    bytes.len() as u64 == asset.size
        && bytes.starts_with(b"wOF2")
        && format!("{:x}", Sha256::digest(bytes)) == asset.digest
}
fn read_asset(root: &Path, asset: &FontAsset) -> Result<Vec<u8>, String> {
    let path = root.join(asset.name);
    if !safe_directory(root)
        || !regular_file(&path)
        || fs::metadata(&path).map_err(|_| "字体文件不可用")?.len() != asset.size
    {
        return Err("推荐字体尚未下载完整，请重新下载".into());
    }
    let bytes = fs::read(path).map_err(|_| "无法读取推荐字体")?;
    if !verify_bytes(asset, &bytes) {
        return Err("推荐字体校验失败，请重新下载".into());
    }
    Ok(bytes)
}
pub fn is_ready(root: &Path) -> bool {
    ASSETS.iter().all(|asset| read_asset(root, asset).is_ok())
}

/// The protocol exposes only these three verified assets, never arbitrary paths.
pub fn serve(root: &Path, path: &str) -> tauri::http::Response<Vec<u8>> {
    let bytes = path
        .strip_prefix('/')
        .and_then(|name| ASSETS.iter().find(|asset| asset.name == name))
        .and_then(|asset| read_asset(root, asset).ok());
    let builder = tauri::http::Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff");
    match bytes {
        Some(bytes) => builder
            .header("Content-Type", "font/woff2")
            .body(bytes)
            .unwrap(),
        None => builder.status(404).body(Vec::new()).unwrap(),
    }
}
fn publish(app: &AppHandle, value: FontProgress) {
    if let Ok(mut progress) = app.state::<FontManager>().progress.lock() {
        *progress = value.clone();
    }
    let _ = app.emit(EVENT, value);
}

#[tauri::command]
pub async fn get_recommended_font_status(app: AppHandle) -> Result<FontProgress, String> {
    let snapshot = app.state::<FontManager>().snapshot()?;
    if app.state::<FontManager>().busy.load(Ordering::Acquire) {
        return Ok(snapshot);
    }
    let root = root(&app)?;
    let ready = tauri::async_runtime::spawn_blocking(move || is_ready(&root))
        .await
        .map_err(|error| error.to_string())?;
    Ok(if ready {
        FontProgress::new("ready", total(), snapshot.message)
    } else if snapshot.phase == "error" {
        snapshot
    } else {
        FontProgress::new("idle", 0, None)
    })
}

fn prepare_root(root: &Path) -> Result<(), String> {
    let parent = root.parent().ok_or("字体缓存路径无效")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建字体缓存目录")?;
    if !safe_directory(parent) {
        return Err("字体缓存目录不可用".into());
    }
    fs::create_dir_all(root).map_err(|_| "无法创建字体缓存目录")?;
    if !safe_directory(root) {
        return Err("字体缓存目录不可用".into());
    }
    Ok(())
}
fn prepare_partial(root: &Path, asset: &FontAsset) -> Result<(PathBuf, fs::File), String> {
    let path = root.join(format!("{}.part", asset.name));
    if path.try_exists().map_err(|_| "无法访问字体缓存")? {
        if !regular_file(&path) {
            return Err("字体临时文件路径不可用".into());
        }
        fs::remove_file(&path).map_err(|_| "无法清理未完成的字体下载")?;
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "无法写入字体缓存")?;
    Ok((path, file))
}
fn commit_partial(root: &Path, asset: &FontAsset, partial: &Path) -> Result<(), String> {
    let bytes = fs::read(partial).map_err(|_| "无法读取已下载字体")?;
    if !verify_bytes(asset, &bytes) {
        let _ = fs::remove_file(partial);
        return Err("字体完整性校验失败，请重新下载".into());
    }
    let final_path = root.join(asset.name);
    if final_path.try_exists().map_err(|_| "无法访问字体缓存")? {
        if !regular_file(&final_path) {
            return Err("字体文件路径不可用".into());
        }
        fs::remove_file(&final_path).map_err(|_| "无法替换损坏的字体缓存")?;
    }
    fs::rename(partial, final_path).map_err(|_| "无法保存已校验的字体".into())
}
async fn download_files(
    root: &Path,
    source: &str,
    assets: &[FontAsset<'_>],
    mut notify: impl FnMut(&str, u64),
) -> Result<(), String> {
    prepare_root(root)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("In-Line-Recommended-Font/0.5.0")
        .build()
        .map_err(|_| "无法初始化字体下载")?;
    let mut completed = 0;
    for asset in assets {
        if read_asset(root, asset).is_ok() {
            completed += asset.size;
            notify("downloading", completed);
            continue;
        }
        let response = client
            .get(format!("{source}/{}", asset.name))
            .send()
            .await
            .map_err(|_| "字体下载失败，请检查网络后重试")?;
        if !response.status().is_success() {
            return Err(format!(
                "字体下载失败（HTTP {}），请稍后重试",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|size| size != asset.size)
        {
            return Err("字体文件大小与预期不一致".into());
        }
        let (partial, mut file) = prepare_partial(root, asset)?;
        let mut received = 0;
        let mut last_emit = Instant::now();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "字体下载中断，请重新下载")?;
            received += chunk.len() as u64;
            if received > asset.size {
                return Err("字体下载超过预期大小".into());
            }
            file.write_all(&chunk)
                .map_err(|_| "无法写入字体缓存，请检查磁盘空间")?;
            if last_emit.elapsed() >= Duration::from_millis(120) {
                notify("downloading", completed + received);
                last_emit = Instant::now();
            }
        }
        file.sync_all().map_err(|_| "无法完成字体缓存写入")?;
        drop(file);
        notify("verifying", completed + received);
        commit_partial(root, asset, &partial)?;
        completed += asset.size;
    }
    Ok(())
}
fn write_notice(root: &Path, name: &str, text: &str) -> Result<(), String> {
    let path = root.join(name);
    if fs::symlink_metadata(&path).is_ok() && !regular_file(&path) {
        return Err("字体来源文件路径不可用".into());
    }
    fs::write(path, text).map_err(|_| "无法保存字体许可证和来源".into())
}
async fn download(app: &AppHandle, root: &Path) -> Result<(), String> {
    download_files(root, SOURCE, &ASSETS, |phase, bytes| {
        publish(app, FontProgress::new(phase, bytes, None))
    })
    .await?;
    write_notice(root, "OFL.txt", include_str!("../../docs/fonts/OFL.txt"))?;
    write_notice(
        root,
        "source.json",
        include_str!("../../docs/fonts/source.json"),
    )?;
    Ok(())
}

#[tauri::command]
pub fn download_recommended_font(app: AppHandle) -> Result<(), String> {
    let root = root(&app)?;
    let manager = app.state::<FontManager>();
    let revision_guard = manager
        .selection_revision
        .lock()
        .map_err(|_| "字体设置不可用")?;
    let previous = app
        .state::<crate::database::Database>()
        .settings()?
        .remove("ui_font_family")
        .unwrap_or_default();
    let revision = *revision_guard;
    drop(revision_guard);
    if manager.busy.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    publish(&app, FontProgress::new("downloading", 0, None));
    tauri::async_runtime::spawn(async move {
        let result = async {
            download(&app, &root).await?;
            let db = app.state::<crate::database::Database>();
            // A newer explicit font choice made during download takes priority.
            let manager = app.state::<FontManager>();
            let mut current_revision = manager
                .selection_revision
                .lock()
                .map_err(|_| "字体设置不可用")?;
            if *current_revision == revision
                && db.settings()?.remove("ui_font_family").unwrap_or_default() == previous
            {
                db.set_setting("ui_font_family".into(), FAMILY.into())?;
                *current_revision += 1;
                let _ = app.emit("data-changed", ());
            }
            Ok::<(), String>(())
        }
        .await;
        match result {
            Ok(()) => publish(&app, FontProgress::new("ready", total(), None)),
            Err(message) => {
                let downloaded = app
                    .state::<FontManager>()
                    .snapshot()
                    .map(|value| value.downloaded_bytes)
                    .unwrap_or(0);
                publish(&app, FontProgress::new("error", downloaded, Some(message)));
            }
        }
        app.state::<FontManager>()
            .busy
            .store(false, Ordering::Release);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[tokio::test]
    async fn interrupted_download_keeps_verified_weights_and_retry_uses_them() {
        let root = std::env::temp_dir().join(format!(
            "inline-font-network-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let bytes = b"wOF2-network-test";
        let digest = format!("{:x}", Sha256::digest(bytes));
        let assets = [
            FontAsset {
                name: "one.woff2",
                size: bytes.len() as u64,
                digest: &digest,
            },
            FontAsset {
                name: "two.woff2",
                size: bytes.len() as u64,
                digest: &digest,
            },
        ];
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut paths = Vec::new();
            for body in [bytes.as_slice(), b"wOF2".as_slice(), bytes.as_slice()] {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        other => panic!("fixture connection failed: {other:?}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                socket.set_nonblocking(false).unwrap();
                let mut request = [0; 2048];
                let count = socket.read(&mut request).unwrap();
                paths.push(
                    String::from_utf8_lossy(&request[..count])
                        .lines()
                        .next()
                        .unwrap()
                        .to_string(),
                );
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                )
                .unwrap();
                socket.write_all(body).unwrap();
            }
            paths
        });
        assert!(download_files(&root, &url, &assets, |_, _| {})
            .await
            .is_err());
        assert!(read_asset(&root, &assets[0]).is_ok());
        assert!(read_asset(&root, &assets[1]).is_err());
        fs::write(root.join("unrelated.txt"), b"keep").unwrap();
        let mut progress = Vec::new();
        download_files(&root, &url, &assets, |phase, bytes| {
            progress.push((phase.to_string(), bytes))
        })
        .await
        .unwrap();
        assert!(read_asset(&root, &assets[1]).is_ok());
        let paths = server.join().unwrap();
        assert_eq!(
            paths,
            [
                "GET /one.woff2 HTTP/1.1",
                "GET /two.woff2 HTTP/1.1",
                "GET /two.woff2 HTTP/1.1"
            ]
        );
        assert_eq!(progress.last().unwrap().1, bytes.len() as u64 * 2);
        assert_eq!(fs::read(root.join("unrelated.txt")).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pinned_assets_are_below_ten_mb_and_only_expected_paths_are_exposed() {
        assert_eq!(total(), 25443648);
        assert!(ASSETS
            .iter()
            .all(|asset| asset.size < 10_000_000 && asset.digest.len() == 64));
        for path in [
            "/../OFL.txt",
            "/OFL.txt",
            "/%2e%2e/secret",
            "/SarasaUiSC-Regular.woff2/extra",
        ] {
            assert_eq!(serve(Path::new("not-present"), path).status(), 404);
        }
        assert!(!is_ready(Path::new("not-present")));
    }
    #[test]
    fn corrupt_partial_is_rejected_and_valid_file_is_committed_without_touching_others() {
        let root = std::env::temp_dir().join(format!(
            "inline-font-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        prepare_root(&root).unwrap();
        fs::write(root.join("unrelated.txt"), b"keep").unwrap();
        let bytes = b"wOF2-font-test";
        let digest = format!("{:x}", Sha256::digest(bytes));
        let asset = FontAsset {
            name: "test.woff2",
            size: bytes.len() as u64,
            digest: &digest,
        };
        let (partial, mut file) = prepare_partial(&root, &asset).unwrap();
        file.write_all(b"corrupt").unwrap();
        drop(file);
        assert!(commit_partial(&root, &asset, &partial).is_err());
        assert!(!partial.exists());
        assert!(!root.join(asset.name).exists());
        let (partial, mut file) = prepare_partial(&root, &asset).unwrap();
        file.write_all(bytes).unwrap();
        drop(file);
        commit_partial(&root, &asset, &partial).unwrap();
        assert_eq!(read_asset(&root, &asset).unwrap(), bytes);
        fs::write(root.join(asset.name), b"damaged").unwrap();
        assert!(read_asset(&root, &asset).is_err());
        assert_eq!(fs::read(root.join("unrelated.txt")).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
