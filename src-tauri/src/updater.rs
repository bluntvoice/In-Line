use futures_util::StreamExt;
use reqwest::{header, redirect::Policy, Client, StatusCode, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/bluntvoice/In-Line/releases/latest";
const UPDATE_EVENT: &str = "update-progress";
const UPDATE_WINDOW: &str = "update-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub status: String,
    pub local_version: String,
    pub remote_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub version: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<u8>,
    pub message: Option<String>,
}

impl Default for UpdateProgress {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            version: None,
            downloaded_bytes: 0,
            total_bytes: None,
            percent: None,
            message: None,
        }
    }
}

impl UpdateProgress {
    fn active(&self) -> bool {
        matches!(
            self.phase.as_str(),
            "checking" | "downloading" | "verifying" | "launching"
        )
    }
}

pub struct UpdateManager(Mutex<UpdateProgress>);

impl Default for UpdateManager {
    fn default() -> Self {
        Self(Mutex::new(UpdateProgress::default()))
    }
}

impl UpdateManager {
    fn get(&self) -> UpdateProgress {
        self.0
            .lock()
            .unwrap_or_else(|value| value.into_inner())
            .clone()
    }

    fn set(&self, progress: UpdateProgress) {
        *self.0.lock().unwrap_or_else(|value| value.into_inner()) = progress;
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
    url: String,
    html_url: String,
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Clone)]
struct DownloadPlan {
    version: String,
    asset_name: String,
    download_url: String,
    expected_digest: String,
}

fn parse_version(value: &str) -> Result<Version, String> {
    Version::parse(value.trim().trim_start_matches(['v', 'V'])).map_err(|_| "版本号格式无效".into())
}

fn validate_release(release: &GitHubRelease) -> Result<Version, String> {
    if release.draft || release.prerelease {
        return Err("暂未找到最新正式版本".into());
    }
    let api = Url::parse(&release.url).map_err(|_| "GitHub Release 来源无效")?;
    let html = Url::parse(&release.html_url).map_err(|_| "GitHub Release 来源无效")?;
    let api_path = api.path().trim_end_matches('/');
    let html_prefix = "/bluntvoice/In-Line/releases/tag/";
    if api.scheme() != "https"
        || api.host_str() != Some("api.github.com")
        || !api_path.starts_with("/repos/bluntvoice/In-Line/releases/")
        || html.scheme() != "https"
        || html.host_str() != Some("github.com")
        || !html.path().starts_with(html_prefix)
    {
        return Err("GitHub Release 来源无效".into());
    }
    parse_version(&release.tag_name)
}

fn validate_asset_url(value: &str, tag: &str, name: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|_| "更新安装包下载地址无效")?;
    let expected = format!("/bluntvoice/In-Line/releases/download/{tag}/{name}");
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.path() != expected
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("更新安装包下载地址不属于官方 GitHub Release".into());
    }
    Ok(())
}

fn parse_sha256_digest(value: Option<&str>) -> Result<String, String> {
    let value = value.ok_or("更新安装包缺少 SHA-256 校验信息，已拒绝自动安装")?;
    let (algorithm, digest) = value
        .split_once(':')
        .ok_or("更新安装包 SHA-256 校验信息无效")?;
    if !algorithm.eq_ignore_ascii_case("sha256")
        || digest.len() != 64
        || !digest.bytes().all(|value| value.is_ascii_hexdigit())
    {
        return Err("更新安装包 SHA-256 校验信息无效".into());
    }
    Ok(digest.to_ascii_lowercase())
}

fn select_windows_asset(
    release: &GitHubRelease,
    version: &Version,
) -> Result<DownloadPlan, String> {
    let version_text = version.to_string().to_ascii_lowercase();
    let matches = release
        .assets
        .iter()
        .filter(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.ends_with(".exe")
                && name.contains(&version_text)
                && name.contains("x64")
                && name.contains("setup")
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err("未找到适用于当前系统的更新安装包".into());
    }
    if matches.len() != 1 {
        return Err("发现多个可能的更新安装包，已停止自动下载".into());
    }
    let asset = matches[0];
    if Path::new(&asset.name)
        .file_name()
        .and_then(|value| value.to_str())
        != Some(asset.name.as_str())
    {
        return Err("更新安装包文件名无效".into());
    }
    validate_asset_url(&asset.browser_download_url, &release.tag_name, &asset.name)?;
    Ok(DownloadPlan {
        version: version.to_string(),
        asset_name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
        expected_digest: parse_sha256_digest(asset.digest.as_deref())?,
    })
}

fn allowed_redirect_host(host: Option<&str>) -> bool {
    matches!(
        host,
        Some("github.com")
            | Some("api.github.com")
            | Some("objects.githubusercontent.com")
            | Some("release-assets.githubusercontent.com")
    )
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("In-Line/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 8 {
                return attempt.error("更新下载重定向次数过多");
            }
            if allowed_redirect_host(attempt.url().host_str()) {
                attempt.follow()
            } else {
                attempt.error("更新下载被重定向到不受信任的地址")
            }
        }))
        .build()
        .map_err(|_| "无法初始化安全网络连接".into())
}

fn status_error(status: StatusCode, checking: bool) -> String {
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return "GitHub 请求暂时受限，请稍后重试".into();
    }
    if checking && status == StatusCode::NOT_FOUND {
        return "暂未找到正式发布版本".into();
    }
    if status.is_server_error() {
        return "GitHub 服务暂时不可用，请稍后重试".into();
    }
    if checking {
        "检查更新失败，请稍后重试".into()
    } else {
        "更新下载失败，请稍后重试".into()
    }
}

async fn fetch_release(client: &Client) -> Result<GitHubRelease, String> {
    let response = client
        .get(LATEST_RELEASE_API)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                String::from("检查更新超时，请稍后重试")
            } else {
                String::from("检查更新失败，请确认网络连接后重试")
            }
        })?;
    if !response.status().is_success() {
        return Err(status_error(response.status(), true));
    }
    response
        .json::<GitHubRelease>()
        .await
        .map_err(|_| "无法解析 GitHub 最新版本信息".into())
}

fn updates_root() -> PathBuf {
    std::env::temp_dir().join("In-Line").join("updates")
}

fn clear_old_updates(root: &Path) -> Result<(), String> {
    if root.exists() {
        fs::remove_dir_all(root).map_err(|error| format!("无法清理旧更新临时文件：{error}"))?;
    }
    Ok(())
}

fn prepare_download_target(root: &Path, plan: &DownloadPlan) -> Result<(PathBuf, PathBuf), String> {
    let directory = root.join(format!("v{}", plan.version));
    fs::create_dir_all(&directory).map_err(|_| "无法创建更新临时目录")?;
    let final_path = directory.join(&plan.asset_name);
    let partial_path = directory.join(format!("{}.part", plan.asset_name));
    for path in [&partial_path, &final_path] {
        if path.exists() {
            fs::remove_file(path).map_err(|_| "无法清理旧的更新临时文件")?;
        }
    }
    Ok((partial_path, final_path))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|_| "无法读取下载完成的更新包")?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| "更新包校验失败")?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn progress_percent(downloaded: u64, total: Option<u64>) -> Option<u8> {
    total
        .filter(|value| *value > 0)
        .map(|value| ((downloaded.saturating_mul(100) / value).min(100)) as u8)
}

fn set_progress(app: &AppHandle, progress: UpdateProgress) {
    app.state::<UpdateManager>().set(progress.clone());
    let _ = app.emit(UPDATE_EVENT, progress);
}

pub fn show_update_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(UPDATE_WINDOW)
        .ok_or("无法打开更新进度窗口")?;
    let monitor = app
        .get_webview_window("main")
        .and_then(|value| value.current_monitor().ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let size = window.outer_size().map_err(|error| error.to_string())?;
        let margin = (16_f64 * monitor.scale_factor()).round() as i32;
        let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())
}

async fn download_and_install(
    app: AppHandle,
    client: Client,
    plan: DownloadPlan,
) -> Result<(), String> {
    let (partial_path, final_path) = prepare_download_target(&updates_root(), &plan)?;
    let response = client
        .get(&plan.download_url)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                String::from("更新下载超时，请稍后重试")
            } else {
                String::from("更新下载失败，请确认网络连接后重试")
            }
        })?;
    if !response.status().is_success() {
        return Err(status_error(response.status(), false));
    }
    if !allowed_redirect_host(response.url().host_str()) {
        return Err("更新下载被重定向到不受信任的地址".into());
    }
    let total = response.content_length();
    let mut file = File::create(&partial_path).map_err(|_| "无法写入更新临时目录")?;
    let mut stream = response.bytes_stream();
    let mut downloaded = 0_u64;
    let mut last_emit = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "更新下载中断，请重新下载")?;
        file.write_all(&chunk).map_err(|_| "无法写入更新临时目录")?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(120) {
            set_progress(
                &app,
                UpdateProgress {
                    phase: "downloading".into(),
                    version: Some(plan.version.clone()),
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                    percent: progress_percent(downloaded, total),
                    message: None,
                },
            );
            last_emit = Instant::now();
        }
    }
    file.flush().map_err(|_| "无法完成更新包写入")?;
    file.sync_all().map_err(|_| "无法完成更新包写入")?;
    drop(file);
    set_progress(
        &app,
        UpdateProgress {
            phase: "verifying".into(),
            version: Some(plan.version.clone()),
            downloaded_bytes: downloaded,
            total_bytes: total,
            percent: progress_percent(downloaded, total),
            message: None,
        },
    );
    let actual_digest = sha256_file(&partial_path)?;
    if actual_digest != plan.expected_digest {
        let _ = fs::remove_file(&partial_path);
        return Err("更新包校验失败，请重新下载".into());
    }
    fs::rename(&partial_path, &final_path).map_err(|_| "无法保存已校验的更新安装包")?;
    set_progress(
        &app,
        UpdateProgress {
            phase: "launching".into(),
            version: Some(plan.version),
            downloaded_bytes: downloaded,
            total_bytes: total,
            percent: Some(100),
            message: None,
        },
    );
    let mut child = Command::new(&final_path)
        .spawn()
        .map_err(|_| "无法启动安装程序，请重新尝试")?;
    std::thread::sleep(Duration::from_millis(450));
    if let Some(status) = child.try_wait().map_err(|_| "无法确认安装程序启动状态")? {
        if !status.success() {
            return Err("无法启动安装程序，请重新尝试".into());
        }
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let current = app.state::<UpdateManager>().get();
    if current.active() {
        show_update_window(&app)?;
        return Ok(UpdateCheckResponse {
            status: "downloading".into(),
            local_version: env!("CARGO_PKG_VERSION").into(),
            remote_version: current.version,
        });
    }
    set_progress(
        &app,
        UpdateProgress {
            phase: "checking".into(),
            ..UpdateProgress::default()
        },
    );
    // 该目录完全属于 In Line 更新器。清理失败不阻止本次检查，后续创建目标目录时仍会给出明确错误。
    let _ = clear_old_updates(&updates_root());
    let client = match http_client() {
        Ok(value) => value,
        Err(error) => {
            app.state::<UpdateManager>().set(UpdateProgress::default());
            return Err(error);
        }
    };
    let release = match fetch_release(&client).await {
        Ok(value) => value,
        Err(error) => {
            app.state::<UpdateManager>().set(UpdateProgress::default());
            return Err(error);
        }
    };
    let remote = match validate_release(&release) {
        Ok(value) => value,
        Err(error) => {
            app.state::<UpdateManager>().set(UpdateProgress::default());
            return Err(error);
        }
    };
    let local = match parse_version(env!("CARGO_PKG_VERSION")) {
        Ok(value) => value,
        Err(error) => {
            app.state::<UpdateManager>().set(UpdateProgress::default());
            return Err(error);
        }
    };
    if remote <= local {
        app.state::<UpdateManager>().set(UpdateProgress::default());
        return Ok(UpdateCheckResponse {
            status: "up_to_date".into(),
            local_version: local.to_string(),
            remote_version: Some(remote.to_string()),
        });
    }
    let plan = match select_windows_asset(&release, &remote) {
        Ok(value) => value,
        Err(error) => {
            app.state::<UpdateManager>().set(UpdateProgress::default());
            return Err(error);
        }
    };
    set_progress(
        &app,
        UpdateProgress {
            phase: "downloading".into(),
            version: Some(plan.version.clone()),
            ..UpdateProgress::default()
        },
    );
    if let Err(error) = show_update_window(&app) {
        app.state::<UpdateManager>().set(UpdateProgress::default());
        return Err(error);
    }
    let background_app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = download_and_install(background_app.clone(), client, plan).await {
            let progress = background_app.state::<UpdateManager>().get();
            if let Some(path) = progress.version.as_deref() {
                let _ = fs::remove_dir_all(updates_root().join(format!("v{path}")));
            }
            set_progress(
                &background_app,
                UpdateProgress {
                    phase: "failed".into(),
                    version: progress.version,
                    downloaded_bytes: progress.downloaded_bytes,
                    total_bytes: progress.total_bytes,
                    percent: progress.percent,
                    message: Some(error),
                },
            );
            let _ = show_update_window(&background_app);
        }
    });
    Ok(UpdateCheckResponse {
        status: "downloading".into(),
        local_version: local.to_string(),
        remote_version: Some(remote.to_string()),
    })
}

#[tauri::command]
pub fn get_update_progress(state: tauri::State<UpdateManager>) -> UpdateProgress {
    state.get()
}

#[tauri::command]
pub fn show_update_progress(app: AppHandle) -> Result<(), String> {
    show_update_window(&app)
}

#[tauri::command]
pub fn hide_update_progress(app: AppHandle) -> Result<(), String> {
    app.get_webview_window(UPDATE_WINDOW)
        .ok_or("无法定位更新进度窗口")?
        .hide()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: Vec<GitHubAsset>) -> GitHubRelease {
        GitHubRelease {
            url: "https://api.github.com/repos/bluntvoice/In-Line/releases/123".into(),
            html_url: format!("https://github.com/bluntvoice/In-Line/releases/tag/{tag}"),
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            assets,
        }
    }

    fn asset(name: &str, digest: Option<&str>) -> GitHubAsset {
        GitHubAsset {
            name: name.into(),
            browser_download_url: format!(
                "https://github.com/bluntvoice/In-Line/releases/download/v0.2.3/{name}"
            ),
            digest: digest.map(str::to_string),
        }
    }

    #[test]
    fn compares_semver_numerically_and_strips_v_prefix() {
        assert!(parse_version("v0.2.4").unwrap() > parse_version("0.2.3").unwrap());
        assert!(parse_version("0.2.10").unwrap() > parse_version("0.2.9").unwrap());
        assert!(parse_version("1.0.0").unwrap() > parse_version("0.99.9").unwrap());
        assert_eq!(
            parse_version("v0.2.3").unwrap(),
            parse_version("0.2.3").unwrap()
        );
        assert!(parse_version("0.2.2").unwrap() < parse_version("0.2.3").unwrap());
    }

    #[test]
    fn rejects_draft_and_prerelease_releases() {
        assert_eq!(
            validate_release(&release("v0.2.3", vec![])).unwrap(),
            parse_version("0.2.3").unwrap()
        );
        let mut draft = release("v0.2.3", vec![]);
        draft.draft = true;
        assert!(validate_release(&draft).is_err());
        let mut prerelease = release("v0.2.3", vec![]);
        prerelease.prerelease = true;
        assert!(validate_release(&prerelease).is_err());
    }

    #[test]
    fn selects_only_the_matching_x64_nsis_asset() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        let value = release(
            "v0.2.3",
            vec![
                asset("In.Line_0.2.3_x86-setup.exe", Some(&digest)),
                asset("In.Line_0.2.3_x64-setup.exe", Some(&digest)),
                asset("In.Line_0.2.3_x64-portable.exe", Some(&digest)),
            ],
        );
        assert_eq!(
            select_windows_asset(&value, &parse_version("0.2.3").unwrap())
                .unwrap()
                .asset_name,
            "In.Line_0.2.3_x64-setup.exe"
        );
        assert!(
            select_windows_asset(&release("v0.2.3", vec![]), &parse_version("0.2.3").unwrap())
                .is_err()
        );
        let duplicate = release(
            "v0.2.3",
            vec![
                asset("In.Line_0.2.3_x64-setup.exe", Some(&digest)),
                asset("In.Line_0.2.3_x64-setup-copy.exe", Some(&digest)),
            ],
        );
        assert!(select_windows_asset(&duplicate, &parse_version("0.2.3").unwrap()).is_err());
    }

    #[test]
    fn rejects_untrusted_release_and_asset_urls() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        let mut wrong_release = release("v0.2.3", vec![]);
        wrong_release.html_url = "https://github.com/another/repository/releases/tag/v0.2.3".into();
        assert!(validate_release(&wrong_release).is_err());

        let mut wrong_asset = asset("In.Line_0.2.3_x64-setup.exe", Some(&digest));
        wrong_asset.browser_download_url = "https://example.com/In.Line_0.2.3_x64-setup.exe".into();
        assert!(select_windows_asset(
            &release("v0.2.3", vec![wrong_asset]),
            &parse_version("0.2.3").unwrap()
        )
        .is_err());
    }

    #[test]
    fn requires_one_valid_sha256_digest() {
        assert_eq!(
            parse_sha256_digest(Some(&format!("SHA256:{}", "AB".repeat(32)))).unwrap(),
            "ab".repeat(32)
        );
        assert!(parse_sha256_digest(None).is_err());
        assert!(parse_sha256_digest(Some("sha512:abcd")).is_err());
        assert!(parse_sha256_digest(Some("sha256:not-hex")).is_err());
    }

    #[test]
    fn verifies_file_digest_and_detects_changes() {
        let root = std::env::temp_dir().join(format!("in-line-update-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("asset.exe");
        fs::write(&path, b"trusted installer").unwrap();
        let first = sha256_file(&path).unwrap();
        assert_eq!(
            first,
            "4dca0f2ee4d185ce634995a24b0704554e91e30d3e9e77d20054934976f6343a"
        );
        fs::write(&path, b"changed installer").unwrap();
        assert_ne!(first, sha256_file(&path).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn handles_unknown_content_length_without_fake_percentage() {
        assert_eq!(progress_percent(1024, None), None);
        assert_eq!(progress_percent(50, Some(100)), Some(50));
    }

    #[test]
    fn replaces_owned_partial_files_and_rejects_unwritable_roots() {
        let root =
            std::env::temp_dir().join(format!("in-line-update-target-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let plan = DownloadPlan {
            version: "0.2.3".into(),
            asset_name: "In.Line_0.2.3_x64-setup.exe".into(),
            download_url: String::new(),
            expected_digest: String::new(),
        };
        let (partial, _) = prepare_download_target(&root, &plan).unwrap();
        fs::write(&partial, b"partial").unwrap();
        let (partial, _) = prepare_download_target(&root, &plan).unwrap();
        assert!(!partial.exists());
        fs::remove_dir_all(&root).unwrap();
        fs::write(&root, b"not a directory").unwrap();
        assert!(prepare_download_target(&root, &plan).is_err());
        fs::remove_file(root).unwrap();
    }
}
