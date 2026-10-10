pub mod database;
pub mod fonts;
pub mod mcp;
pub mod models;
pub mod recommended_font;
pub mod updater;

#[cfg(windows)]
#[tauri::command]
fn mcp_audit_state(db: tauri::State<database::Database>) -> Result<serde_json::Value, String> {
    db.mcp_audit_state()
}
#[cfg(windows)]
#[tauri::command]
fn mcp_resolve_undo(
    app: tauri::AppHandle,
    db: tauri::State<database::Database>,
    request_id: i64,
    approve: bool,
) -> Result<serde_json::Value, String> {
    let result = db
        .mcp_resolve_undo(request_id, approve)
        .map_err(|e| e.to_string())?;
    if approve {
        let _ = emit_change(&app);
    }
    Ok(result)
}

use database::Database;
use models::*;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_window_state::StateFlags;

struct GlobalShortcutStatus(Mutex<GlobalShortcutConfig>);
struct UiScaleState(Mutex<f64>);
struct GlobalShortcutConfig {
    shortcut: String,
    available: bool,
}
const DEFAULT_GLOBAL_SHORTCUT: &str = "Alt+I";

fn validate_global_shortcut(value: &str) -> Result<(), String> {
    if value.len() > 64 || !value.is_ascii() {
        return Err("快捷键格式无效".into());
    }
    let shortcut = value
        .parse::<Shortcut>()
        .map_err(|_| "无法识别该快捷键组合".to_string())?;
    if shortcut.mods.contains(Modifiers::SUPER) {
        return Err("Windows 键组合由系统保留，请改用 Ctrl 或 Alt".into());
    }
    if !shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT)
    {
        return Err("全局快捷键必须包含 Ctrl 或 Alt".into());
    }
    if shortcut.mods.contains(Modifiers::ALT) && shortcut.key == Code::F4 {
        return Err("Alt+F4 是 Windows 关闭窗口快捷键，不能使用".into());
    }
    Ok(())
}

fn valid_global_shortcut(value: &str) -> bool {
    validate_global_shortcut(value).is_ok()
}

fn emit_change(app: &tauri::AppHandle) -> Result<(), String> {
    app.emit("data-changed", ())
        .map_err(|error| error.to_string())
}
fn emit_floating_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("floating") {
        if let Ok(visible) = window.is_visible() {
            let _ = app.emit("floating-visibility-changed", visible);
        }
    }
}
fn show_main(app: &tauri::AppHandle) {
    for label in ["floating", "quick-add"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
    emit_floating_visibility(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn show_floating(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("floating") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    emit_floating_visibility(app);
}
fn show_quick_add(app: &tauri::AppHandle) {
    let _ = app.emit_to("quick-add", "new-task", ());
    if let Some(window) = app.get_webview_window("quick-add") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn bootstrap(db: State<Database>) -> Result<BootstrapData, String> {
    db.bootstrap()
}
#[tauri::command]
fn list_tasks(db: State<Database>, view: TaskView) -> Result<Vec<LegalTask>, String> {
    db.list_tasks(view)
}
#[tauri::command]
fn save_task(
    app: tauri::AppHandle,
    db: State<Database>,
    task: TaskInput,
) -> Result<LegalTask, String> {
    let value = db.save_task(task)?;
    emit_change(&app)?;
    Ok(value)
}
#[tauri::command]
fn create_subtask(
    app: tauri::AppHandle,
    db: State<Database>,
    input: CreateSubtaskInput,
) -> Result<LegalTask, String> {
    let task = db.create_subtask(input)?;
    emit_change(&app)?;
    Ok(task)
}
#[tauri::command]
fn set_task_ticket_color(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
    color: Option<String>,
) -> Result<(), String> {
    db.set_task_ticket_color(id, color)?;
    emit_change(&app)
}
#[tauri::command]
fn set_task_status(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
    status: String,
) -> Result<(), String> {
    db.set_status(id, status)?;
    emit_change(&app)
}
#[tauri::command]
fn set_task_urgent(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
    is_urgent: bool,
    requester: String,
    reason: String,
) -> Result<(), String> {
    db.set_urgent(id, is_urgent, requester, reason)?;
    emit_change(&app)
}
#[tauri::command]
fn move_task(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
    direction: MoveDirection,
) -> Result<(), String> {
    db.move_task(id, direction)?;
    emit_change(&app)
}
#[tauri::command]
fn delete_task(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.soft_delete(id)?;
    emit_change(&app)
}
#[tauri::command]
fn restore_task(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.restore(id)?;
    emit_change(&app)
}
#[tauri::command]
fn permanently_delete_tasks(
    app: tauri::AppHandle,
    db: State<Database>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    let deleted = db.permanently_delete_tasks(ids)?;
    emit_change(&app)?;
    Ok(deleted)
}
#[tauri::command]
fn empty_trash(app: tauri::AppHandle, db: State<Database>) -> Result<usize, String> {
    let deleted = db.empty_trash()?;
    emit_change(&app)?;
    Ok(deleted)
}
#[tauri::command]
fn archive_task(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.archive(id)?;
    emit_change(&app)
}

#[tauri::command]
fn get_subtask_completion_state(
    db: State<Database>,
    task_id: i64,
) -> Result<Option<SubtaskCompletionState>, String> {
    db.subtask_completion_state(task_id)
}

#[tauri::command]
fn complete_task(
    app: tauri::AppHandle,
    db: State<Database>,
    input: CompleteTaskInput,
) -> Result<CompleteTaskResult, String> {
    let result = db.complete_task(input)?;
    emit_change(&app)?;
    Ok(result)
}

#[tauri::command]
fn archive_task_group(
    app: tauri::AppHandle,
    db: State<Database>,
    input: ArchiveTaskInput,
) -> Result<ArchiveTaskResult, String> {
    let result = db.archive_task_group(input)?;
    emit_change(&app)?;
    Ok(result)
}

#[tauri::command]
fn delete_task_group(
    app: tauri::AppHandle,
    db: State<Database>,
    input: DeleteTaskInput,
) -> Result<DeleteTaskResult, String> {
    let result = db.delete_task_group(input)?;
    emit_change(&app)?;
    Ok(result)
}

fn persisted_window_state_flags() -> StateFlags {
    StateFlags::SIZE
        | StateFlags::POSITION
        | StateFlags::MAXIMIZED
        | StateFlags::DECORATIONS
        | StateFlags::FULLSCREEN
}
#[tauri::command]
fn merge_tasks(
    app: tauri::AppHandle,
    db: State<Database>,
    input: MergeTaskInput,
) -> Result<(), String> {
    db.merge_tasks(input)?;
    emit_change(&app)
}
#[tauri::command]
fn resolve_import_conflict(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
) -> Result<(), String> {
    db.resolve_import_conflict(id)?;
    emit_change(&app)
}
#[tauri::command]
fn list_parent_task_candidates(
    db: State<Database>,
    task_id: i64,
) -> Result<Vec<LegalTask>, String> {
    db.list_parent_task_candidates(task_id)
}
#[tauri::command]
fn list_subtasks(db: State<Database>, parent_task_id: i64) -> Result<Vec<LegalTask>, String> {
    db.list_subtasks(parent_task_id)
}
#[tauri::command]
fn set_parent_task(
    app: tauri::AppHandle,
    db: State<Database>,
    task_id: i64,
    parent_task_id: Option<i64>,
) -> Result<(), String> {
    db.set_parent_task(task_id, parent_task_id)?;
    emit_change(&app)
}
#[tauri::command]
fn reorder_subtasks(
    app: tauri::AppHandle,
    db: State<Database>,
    input: ReorderSubtasksInput,
) -> Result<(), String> {
    db.reorder_subtasks(input)?;
    emit_change(&app)
}
#[tauri::command]
fn get_logs(db: State<Database>, task_id: i64) -> Result<Vec<TaskLog>, String> {
    db.get_logs(task_id)
}
#[tauri::command]
fn get_work_events(db: State<Database>, task_id: i64) -> Result<Vec<TaskWorkEvent>, String> {
    db.list_work_events(task_id)
}
#[tauri::command]
fn void_work_event(
    app: tauri::AppHandle,
    db: State<Database>,
    id: i64,
    confirm_historical_impact: bool,
) -> Result<(), String> {
    db.void_work_event(id, confirm_historical_impact)?;
    emit_change(&app)
}
#[tauri::command]
fn process_round(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.process_round(id)?;
    emit_change(&app)
}
#[tauri::command]
fn complete_round(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.complete_round(id)?;
    emit_change(&app)
}
#[tauri::command]
fn enqueue_task(
    app: tauri::AppHandle,
    db: State<Database>,
    input: QueueInput,
) -> Result<(), String> {
    db.enqueue_task(input)?;
    emit_change(&app)
}
#[tauri::command]
fn reopen_task(
    app: tauri::AppHandle,
    db: State<Database>,
    input: QueueInput,
) -> Result<(), String> {
    db.reopen_task(input)?;
    emit_change(&app)
}
#[tauri::command]
fn get_statistics(
    db: State<Database>,
    start: String,
    end: String,
    timezone_offset_minutes: i32,
) -> Result<StatisticsResult, String> {
    db.statistics(start, end, timezone_offset_minutes)
}
#[tauri::command]
fn get_work_calendar(
    db: State<Database>,
    start: String,
    end: String,
) -> Result<WorkCalendarResult, String> {
    db.work_calendar(start, end)
}
#[tauri::command]
fn get_statistics_details(
    db: State<Database>,
    start: String,
    end: String,
    task_type: String,
) -> Result<Vec<StatisticsDetail>, String> {
    db.statistics_details(start, end, task_type)
}
#[tauri::command]
fn get_statistics_trend_details(
    db: State<Database>,
    start: String,
    end: String,
    result_status: Option<String>,
) -> Result<Vec<StatisticsDetail>, String> {
    db.statistics_trend_details(start, end, result_status)
}
#[tauri::command]
fn add_log(
    app: tauri::AppHandle,
    db: State<Database>,
    task_id: i64,
    content: String,
) -> Result<(), String> {
    db.add_log(task_id, content)?;
    emit_change(&app)
}
#[tauri::command]
fn update_log(
    app: tauri::AppHandle,
    db: State<Database>,
    log_id: i64,
    content: String,
) -> Result<(), String> {
    db.update_log(log_id, content)?;
    emit_change(&app)
}
#[tauri::command]
fn delete_log(app: tauri::AppHandle, db: State<Database>, log_id: i64) -> Result<(), String> {
    db.delete_log(log_id)?;
    emit_change(&app)
}
#[tauri::command]
fn add_master(db: State<Database>, kind: String, name: String) -> Result<MasterData, String> {
    db.add_master(kind, name)
}
#[tauri::command]
fn delete_master(
    app: tauri::AppHandle,
    db: State<Database>,
    kind: String,
    name: String,
) -> Result<MasterData, String> {
    let values = db.delete_master(kind, name)?;
    emit_change(&app)?;
    Ok(values)
}
#[tauri::command]
fn move_master(
    app: tauri::AppHandle,
    db: State<Database>,
    kind: String,
    name: String,
    direction: MoveDirection,
) -> Result<MasterData, String> {
    let values = db.move_master(kind, name, direction)?;
    emit_change(&app)?;
    Ok(values)
}
#[tauri::command]
fn queue_ahead(db: State<Database>, id: i64) -> Result<i64, String> {
    db.queue_ahead(id)
}
#[tauri::command]
fn ticket_snapshot(db: State<Database>, id: i64) -> Result<TicketSnapshot, String> {
    db.ticket_snapshot(id)
}
#[tauri::command]
fn list_backups(db: State<Database>) -> Result<Vec<BackupInfo>, String> {
    db.list_backups()
}
#[tauri::command]
fn create_backup(db: State<Database>) -> Result<BackupInfo, String> {
    db.create_backup("manual")
}
#[tauri::command]
fn import_backup(
    app: tauri::AppHandle,
    db: State<Database>,
    path: String,
) -> Result<BackupInfo, String> {
    let backup = db.import_backup(path)?;
    emit_change(&app)?;
    Ok(backup)
}
#[tauri::command]
fn open_backup_directory(db: State<Database>) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(db.backup_directory())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开备份目录：{error}"))
}
fn mcp_executable() -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("无法定位软件安装目录：{error}"))?
        .with_file_name("in-line-mcp.exe");
    if !executable.is_file() {
        return Err("未找到 MCP 服务程序，请重新安装包含 MCP 功能的新版本".into());
    }
    Ok(executable)
}
#[tauri::command]
fn mcp_connection_guide() -> Result<String, String> {
    let executable = mcp_executable()?;
    Ok(format!(
        "In Line MCP（本机stdio）\n服务程序：{}\n\n新接入流程：选择AI客户端与权限，授权并复制提示词，发送给所选AI。长期凭证只由本机程序处理，不在提示词中显示。Codex当前会话自动接入仍在实机验证，入口暂未开放；既有授权仍可管理或撤销。\n\n工具：get_capabilities、get_report_summary、list_report_items。每页最多100；统一返回status/data/error。常规读取不返回办理自由文本；完整读取需全局及客户端双方启用。当前开发阶段尚无写入工具。\n\n配置与授权验证、当前AI会话连接验证分别记录；禁止用另起实例冒充当前会话自动重载成功。",
        executable.display()
    ))
}

type McpSecurity = std::sync::Arc<mcp::security::Security>;
fn mcp_local(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("请在主界面软件设置管理MCP授权".into());
    }
    Ok(())
}
fn mcp_error(error: mcp::contract::McpError) -> String {
    format!("{}：{}", error.code, error.message)
}
#[tauri::command]
fn mcp_security_state(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
) -> Result<serde_json::Value, String> {
    mcp_local(&window)?;
    let mut view = serde_json::to_value(security.view().map_err(mcp_error)?)
        .map_err(|_| "无法读取授权状态".to_string())?;
    view["onboardingClients"] = serde_json::to_value(mcp::onboarding::presets())
        .map_err(|_| "无法读取客户端状态".to_string())?;
    Ok(view)
}
#[tauri::command]
fn mcp_prepare_onboarding(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    args: mcp::onboarding::PrepareArgs,
) -> Result<mcp::onboarding::Receipt, String> {
    mcp_local(&window)?;
    mcp::onboarding::require_available(&args.client).map_err(mcp_error)?;
    mcp::onboarding::prepare_at(
        &security,
        &mcp::platform::root().map_err(mcp_error)?,
        &mcp_executable()?,
        args,
        chrono::Utc::now().timestamp(),
    )
    .map_err(mcp_error)
}
#[tauri::command]
fn mcp_rotate_onboarding(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    id: String,
) -> Result<mcp::onboarding::Receipt, String> {
    mcp_local(&window)?;
    mcp::onboarding::require_available("codex").map_err(mcp_error)?;
    mcp::onboarding::rotate_at(
        &security,
        &mcp::platform::root().map_err(mcp_error)?,
        &mcp_executable()?,
        &id,
        chrono::Utc::now().timestamp(),
    )
    .map_err(mcp_error)
}
#[tauri::command]
fn mcp_revoke_client(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    id: String,
) -> Result<(), String> {
    mcp_local(&window)?;
    security.revoke(&id).map_err(mcp_error)
}
#[tauri::command]
fn mcp_update_client(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    id: String,
    permissions: mcp::security::Permissions,
    scope: mcp::scope::Scope,
) -> Result<(), String> {
    mcp_local(&window)?;
    security
        .update_client(&id, permissions, scope)
        .map_err(mcp_error)
}
#[tauri::command]
fn mcp_set_groups(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    groups: mcp::security::Permissions,
) -> Result<(), String> {
    mcp_local(&window)?;
    security.set_groups(groups).map_err(mcp_error)
}
#[tauri::command]
fn mcp_set_paused(
    window: tauri::WebviewWindow,
    security: State<McpSecurity>,
    paused: bool,
) -> Result<(), String> {
    mcp_local(&window)?;
    security.pause(paused).map_err(mcp_error)
}
#[tauri::command]
fn delete_backup(app: tauri::AppHandle, db: State<Database>, path: String) -> Result<(), String> {
    db.delete_backup(path)?;
    emit_change(&app)
}
#[tauri::command]
fn cleanup_backups(
    app: tauri::AppHandle,
    db: State<Database>,
) -> Result<BackupCleanupResult, String> {
    let result = db.cleanup_backups()?;
    emit_change(&app)?;
    Ok(result)
}
#[tauri::command]
async fn set_setting(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    key: String,
    value: String,
) -> Result<(), String> {
    let value = if key == "ui_font_family" {
        let root = recommended_font::root(&app)?;
        tauri::async_runtime::spawn_blocking(move || {
            if value == recommended_font::FAMILY {
                if recommended_font::is_ready(&root) {
                    Ok(value)
                } else {
                    Err("请先下载推荐字体".into())
                }
            } else {
                fonts::validate_selection(&value)
            }
        })
        .await
        .map_err(|error| error.to_string())??
    } else {
        value
    };
    if key == "ui_scale" {
        if !matches!(
            value.as_str(),
            "100" | "110" | "120" | "130" | "140" | "150"
        ) {
            return Err("界面大小仅支持 100% 至 150% 的六档设置".into());
        }
        let previous = db
            .settings()?
            .remove("ui_scale")
            .unwrap_or_else(|| "100".into());
        db.set_setting(key, value.clone())?;
        if let Err(error) = apply_ui_scale(&app, &value) {
            let _ = db.set_setting("ui_scale".into(), previous.clone());
            let _ = apply_ui_scale(&app, &previous);
            return Err(error);
        }
        return emit_change(&app);
    }
    if key == "ui_font_family" {
        recommended_font::save_selection(&app, &db, value)?;
    } else {
        db.set_setting(key, value)?;
    }
    emit_change(&app)
}
#[tauri::command]
fn get_ticket_colors(db: State<Database>) -> Result<Option<String>, String> {
    Ok(db.settings()?.remove("ticket_colors"))
}
fn resize_auxiliary_window(
    window: &tauri::WebviewWindow,
    mut width: f64,
    mut height: f64,
) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|_| "无法读取窗口所在显示器")?;
    let dpi = window.scale_factor().map_err(|_| "无法读取窗口缩放")?;
    if let Some(monitor) = &monitor {
        width = width.min(monitor.work_area().size.width as f64 / dpi);
        height = height.min(monitor.work_area().size.height as f64 / dpi);
    }
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|_| "无法调整辅助窗口大小")?;
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let position = window.outer_position().map_err(|_| "无法读取窗口位置")?;
        let max_x = (area.position.x + area.size.width as i32 - (width * dpi).ceil() as i32)
            .max(area.position.x);
        let max_y = (area.position.y + area.size.height as i32 - (height * dpi).ceil() as i32)
            .max(area.position.y);
        window
            .set_position(tauri::PhysicalPosition::new(
                position.x.clamp(area.position.x, max_x),
                position.y.clamp(area.position.y, max_y),
            ))
            .map_err(|_| "无法调整窗口位置")?;
    }
    Ok(())
}
#[tauri::command]
fn resize_floating(app: tauri::AppHandle, mini: bool) -> Result<(), String> {
    let state = app.state::<UiScaleState>();
    let factor = *state.0.lock().map_err(|_| "界面大小设置不可用")?;
    let window = app.get_webview_window("floating").ok_or("悬浮窗尚未创建")?;
    resize_auxiliary_window(
        &window,
        444. * factor,
        (if mini { 72. } else { 564. }) * factor,
    )
}
fn apply_ui_scale(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    let factor = value
        .parse::<f64>()
        .ok()
        .filter(|value| [100., 110., 120., 130., 140., 150.].contains(value))
        .unwrap_or(100.)
        / 100.;
    let state = app.state::<UiScaleState>();
    let mut previous = state.0.lock().map_err(|_| "界面大小设置不可用")?;
    if (*previous - factor).abs() < f64::EPSILON {
        return Ok(());
    }
    let old_factor = *previous;
    *previous = factor;
    // Small fixed-size windows need enough space for their scaled controls.
    if let Some(window) = app.get_webview_window("floating") {
        let height = window
            .inner_size()
            .map_err(|_| "无法读取悬浮窗大小")?
            .height as f64
            / window.scale_factor().map_err(|_| "无法读取悬浮窗缩放")?
            / old_factor;
        let base_height = if height <= 120. { 72. } else { 564. };
        resize_auxiliary_window(&window, 444. * factor, base_height * factor)?;
    }
    if let Some(window) = app.get_webview_window("update-progress") {
        resize_auxiliary_window(&window, 360. * factor, 188. * factor)?;
    }
    for window in app.webview_windows().values() {
        window
            .set_zoom(factor)
            .map_err(|_| "无法调整界面大小，请重试")?;
    }
    *previous = factor;
    Ok(())
}
#[tauri::command]
fn get_ui_scale(app: tauri::AppHandle, db: State<Database>) -> Result<String, String> {
    let value = db
        .settings()?
        .remove("ui_scale")
        .unwrap_or_else(|| "100".into());
    apply_ui_scale(&app, &value)?;
    Ok(value)
}
#[tauri::command]
async fn list_system_fonts() -> Result<Vec<fonts::SystemFont>, String> {
    tauri::async_runtime::spawn_blocking(fonts::system_fonts)
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn get_ui_font_selection(
    app: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<fonts::UiFontSelection, String> {
    let requested = db.settings()?.remove("ui_font_family").unwrap_or_default();
    let root = recommended_font::root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        if requested == recommended_font::FAMILY {
            let ready = recommended_font::is_ready(&root);
            Ok(fonts::UiFontSelection {
                requested,
                effective: if ready {
                    recommended_font::FAMILY.into()
                } else {
                    String::new()
                },
                missing: !ready,
            })
        } else {
            fonts::resolve_selection(requested)
        }
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
fn get_launch_at_login(app: tauri::AppHandle, db: State<Database>) -> Result<bool, String> {
    let actual = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())?;
    let stored = db
        .settings()?
        .get("launch_at_login")
        .and_then(|value| value.parse::<bool>().ok());
    let Some(desired) = stored else {
        db.set_setting("launch_at_login".into(), actual.to_string())?;
        return Ok(actual);
    };
    if desired != actual {
        if desired {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        }
        .map_err(|error| error.to_string())?;
    }
    Ok(desired)
}
#[tauri::command]
fn set_launch_at_login(
    app: tauri::AppHandle,
    db: State<Database>,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|error| error.to_string())?;
    db.set_setting("launch_at_login".into(), enabled.to_string())?;
    emit_change(&app)
}
#[tauri::command]
fn restore_backup(
    app: tauri::AppHandle,
    db: State<Database>,
    path: String,
) -> Result<BackupMergeResult, String> {
    let result = db.restore_backup(path)?;
    if let Some(enabled) = db
        .settings()?
        .get("launch_at_login")
        .and_then(|value| value.parse::<bool>().ok())
    {
        let _ = if enabled {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
    }
    emit_change(&app)?;
    Ok(result)
}
#[tauri::command]
fn copy_ticket_card(db: State<Database>, id: i64) -> Result<LegalTask, String> {
    db.get_task(id)
}
#[tauri::command]
fn open_task_action(
    app: tauri::AppHandle,
    db: State<Database>,
    request: OpenTaskAction,
) -> Result<(), String> {
    match request.action.as_str() {
        "view" | "edit" | "status" | "urgent" | "complete" | "addSubtask" => {
            db.get_task(request.id)?;
            show_main(&app);
            app.emit("task-ui-action", request)
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        "archive" => db.archive(request.id)?,
        "delete" => db.soft_delete(request.id)?,
        "restore" => db.restore(request.id)?,
        _ => return Err("不支持的事项操作".into()),
    }
    emit_change(&app)
}
#[tauri::command]
fn toggle_floating(app: tauri::AppHandle) -> Result<bool, String> {
    let visible = get_floating_visible(app.clone())?;
    set_floating_visible(app, !visible)
}
#[tauri::command]
fn get_floating_visible(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("floating").ok_or("悬浮窗尚未创建")?;
    window.is_visible().map_err(|error| error.to_string())
}
#[tauri::command]
fn set_floating_visible(app: tauri::AppHandle, visible: bool) -> Result<bool, String> {
    let window = app.get_webview_window("floating").ok_or("悬浮窗尚未创建")?;
    if visible {
        window.show()
    } else {
        window.hide()
    }
    .map_err(|error| error.to_string())?;
    let actual = window.is_visible().map_err(|error| error.to_string())?;
    let _ = app.emit("floating-visibility-changed", actual);
    Ok(actual)
}
#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    show_main(&app);
}
#[tauri::command]
fn request_new_task(app: tauri::AppHandle) -> Result<(), String> {
    show_quick_add(&app);
    Ok(())
}

#[tauri::command]
fn global_shortcut_available(status: State<GlobalShortcutStatus>) -> bool {
    status
        .0
        .lock()
        .map(|value| value.available)
        .unwrap_or(false)
}

#[tauri::command]
fn set_global_shortcut(
    app: tauri::AppHandle,
    db: State<Database>,
    status: State<GlobalShortcutStatus>,
    shortcut: String,
) -> Result<(), String> {
    validate_global_shortcut(&shortcut)?;
    let mut current = status.0.lock().map_err(|_| "快捷键状态暂时不可用")?;
    if current.shortcut == shortcut && current.available {
        return Ok(());
    }
    let previous = current.shortcut.clone();
    let previous_available = current.available;
    if previous_available {
        app.global_shortcut()
            .unregister(previous.as_str())
            .map_err(|error| error.to_string())?;
    }
    if let Err(error) = app.global_shortcut().register(shortcut.as_str()) {
        current.available =
            previous_available && app.global_shortcut().register(previous.as_str()).is_ok();
        return Err(format!(
            "快捷键 {shortcut} 已被其他程序占用或无法注册：{error}"
        ));
    }
    if let Err(error) = db.set_setting("global_shortcut".into(), shortcut.clone()) {
        let _ = app.global_shortcut().unregister(shortcut.as_str());
        current.available = app.global_shortcut().register(previous.as_str()).is_ok();
        return Err(format!("保存快捷键设置失败：{error}"));
    }
    current.shortcut = shortcut;
    current.available = true;
    emit_change(&app)
}

#[tauri::command]
fn save_chart_export(path: String, bytes: Vec<u8>) -> Result<(), String> {
    let target = std::path::PathBuf::from(path);
    let is_png = target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err("统计图表只能保存为 PNG 文件".into());
    }
    if bytes.len() < 8 || bytes[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err("统计图表文件无效".into());
    }
    std::fs::write(&target, bytes).map_err(|error| format!("保存统计图表失败：{error}"))
}

pub fn run() {
    let background = std::env::args().any(|arg| arg == "--mcp-background");
    let context = tauri::generate_context!();
    #[cfg(debug_assertions)]
    let context = {
        let mut context = context;
        if let Some(root) = std::env::var_os("IN_LINE_MCP_TEST_DATA_ROOT") {
            use sha2::{Digest, Sha256};
            let suffix = Sha256::digest(root.to_string_lossy().as_bytes())
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>();
            context.config_mut().identifier =
                format!("io.github.bluntvoice.inline.test.{}", &suffix[..16]);
            for window in &mut context.config_mut().app.windows {
                window.data_directory = Some(
                    std::path::PathBuf::from(&root)
                        .join("webviews")
                        .join(&window.label),
                );
            }
        }
        context
    };
    // Invisible windows still default to focus=true in Tauri's WindowConfig.
    // Set both flags before window creation; hiding later cannot undo focus theft.
    let mut context = context;
    if background {
        for window in &mut context.config_mut().app.windows {
            window.visible = false;
            window.focus = false;
        }
    }
    #[cfg(windows)]
    let host_gate = std::sync::Arc::new(
        mcp::platform::HostGate::enter(&context.config().identifier)
            .expect("主程序单实例协调失败，未打开业务库"),
    );
    #[cfg(windows)]
    let setup_gate = host_gate.clone();
    tauri::Builder::default()
        .manage(UiScaleState(Mutex::new(1.)))
        .manage(recommended_font::FontManager::default())
        .register_uri_scheme_protocol("recommended-font", |context, request| {
            let root = recommended_font::root(context.app_handle()).unwrap_or_default();
            recommended_font::serve(&root, request.uri().path())
        })
        .manage(updater::UpdateManager::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--mcp-background") {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        show_quick_add(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(persisted_window_state_flags())
                .build(),
        )
        .setup(move |app| {
            // Single-instance plugin has already resolved secondary launches before opening DB.
            #[cfg(windows)]
            if !setup_gate.owns_database() {
                return Err(
                    std::io::Error::other("已有主程序占有业务库，第二实例未打开数据库").into(),
                );
            }
            let database = Database::open().map_err(std::io::Error::other)?;
            let initial_shortcut = database
                .settings()
                .ok()
                .and_then(|values| values.get("global_shortcut").cloned())
                .filter(|value| valid_global_shortcut(value))
                .unwrap_or_else(|| DEFAULT_GLOBAL_SHORTCUT.into());
            app.manage(database);
            #[cfg(windows)]
            {
                // Corrupt authorization fails closed for MCP while keeping the local UI available.
                if let Ok(security) = mcp::security::Security::open() {
                    let security = std::sync::Arc::new(security);
                    let handle = app.handle().clone();
                    let service_security = security.clone();
                    if mcp::ipc::start(
                        security.clone(),
                        std::sync::Arc::new(move |credentials, tool, args| {
                            let result = mcp::service::execute(
                                &service_security,
                                &handle.state::<Database>(),
                                credentials,
                                tool,
                                args,
                            );
                            if matches!(tool, "mutate_task" | "manage_preferences" | "request_undo")
                                && result.is_ok()
                            {
                                let _ = handle.emit("data-changed", ());
                                let _ = handle.emit("mcp-audit-changed", ());
                            }
                            result
                        }),
                    )
                    .is_err()
                    {
                        eprintln!("MCP本机协调服务不可用，已拒绝客户端访问");
                    }
                    app.manage(security);
                } else {
                    eprintln!("MCP安全存储不可用，已拒绝客户端访问；请在软件内检查");
                }
            }
            let shortcut_available = match app.global_shortcut().register(initial_shortcut.as_str())
            {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("无法注册全局快捷键 {initial_shortcut}：{error}");
                    false
                }
            };
            app.manage(GlobalShortcutStatus(Mutex::new(GlobalShortcutConfig {
                shortcut: initial_shortcut.clone(),
                available: shortcut_available,
            })));
            let open = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
            let add = MenuItem::with_id(app, "add", "新增事项", true, None::<&str>)?;
            let float = MenuItem::with_id(app, "floating", "显示悬浮窗", true, None::<&str>)?;
            let backup = MenuItem::with_id(app, "backup", "立即备份", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&add, &open, &float, &backup, &quit])?;
            let app_icon = app.default_window_icon().cloned();
            if let Some(icon) = app_icon.clone() {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_icon(icon.clone())?;
                }
                if let Some(window) = app.get_webview_window("floating") {
                    window.set_icon(icon.clone())?;
                }
                if let Some(window) = app.get_webview_window("quick-add") {
                    window.set_icon(icon.clone())?;
                }
                if let Some(window) = app.get_webview_window("update-progress") {
                    window.set_icon(icon.clone())?;
                }
            }
            let mut tray_builder = TrayIconBuilder::new();
            if let Some(icon) = app_icon {
                tray_builder = tray_builder.icon(icon);
            }
            let _tray = tray_builder
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "add" => {
                        show_quick_add(app);
                    }
                    "floating" => show_floating(app),
                    "backup" => {
                        if let Some(db) = app.try_state::<Database>() {
                            let _ = db.create_backup("manual");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_floating(tray.app_handle());
                    }
                })
                .build(app)?;
            #[cfg(windows)]
            setup_gate
                .mark_ready()
                .map_err(|e| std::io::Error::other(e.message))?;
            if !background {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    show_floating(window.app_handle());
                }
            }
            if window.label() == "quick-add" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if window.label() == "update-progress" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            list_tasks,
            save_task,
            set_task_ticket_color,
            create_subtask,
            set_task_status,
            set_task_urgent,
            move_task,
            delete_task,
            restore_task,
            permanently_delete_tasks,
            empty_trash,
            archive_task,
            get_subtask_completion_state,
            complete_task,
            archive_task_group,
            delete_task_group,
            merge_tasks,
            resolve_import_conflict,
            list_parent_task_candidates,
            list_subtasks,
            set_parent_task,
            reorder_subtasks,
            get_logs,
            get_work_events,
            void_work_event,
            process_round,
            complete_round,
            enqueue_task,
            reopen_task,
            get_work_calendar,
            get_statistics,
            get_statistics_details,
            get_statistics_trend_details,
            add_log,
            update_log,
            delete_log,
            add_master,
            delete_master,
            move_master,
            queue_ahead,
            ticket_snapshot,
            list_backups,
            create_backup,
            import_backup,
            open_backup_directory,
            mcp_connection_guide,
            mcp_security_state,
            mcp_prepare_onboarding,
            mcp_rotate_onboarding,
            mcp_revoke_client,
            mcp_update_client,
            mcp_set_groups,
            mcp_set_paused,
            mcp_audit_state,
            mcp_resolve_undo,
            delete_backup,
            cleanup_backups,
            set_setting,
            get_ticket_colors,
            list_system_fonts,
            get_ui_font_selection,
            get_ui_scale,
            recommended_font::get_recommended_font_status,
            recommended_font::download_recommended_font,
            get_launch_at_login,
            set_launch_at_login,
            restore_backup,
            copy_ticket_card,
            open_task_action,
            toggle_floating,
            get_floating_visible,
            set_floating_visible,
            resize_floating,
            show_main_window,
            request_new_task,
            global_shortcut_available,
            set_global_shortcut,
            save_chart_export,
            updater::check_for_update,
            updater::get_update_progress,
            updater::show_update_progress,
            updater::hide_update_progress
        ])
        .run(context)
        .expect("In Line 启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_visibility_is_not_restored_on_startup() {
        assert!(!persisted_window_state_flags().contains(StateFlags::VISIBLE));
    }

    #[test]
    fn global_shortcut_validation_accepts_custom_combinations_and_rejects_reserved_ones() {
        assert!(valid_global_shortcut("Ctrl+Alt+K"));
        assert!(valid_global_shortcut("Alt+Shift+8"));
        assert!(valid_global_shortcut("Ctrl+F10"));
        assert!(!valid_global_shortcut("I"));
        assert!(!valid_global_shortcut("Shift+I"));
        assert!(!valid_global_shortcut("Super+I"));
        assert!(!valid_global_shortcut("Alt+F4"));
    }

    #[test]
    fn chart_export_only_writes_png_data_to_png_paths() {
        let path =
            std::env::temp_dir().join(format!("in-line-chart-export-{}.png", std::process::id()));
        let png = vec![137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3];
        save_chart_export(path.to_string_lossy().into_owned(), png.clone()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), png);
        std::fs::remove_file(path).unwrap();
        assert!(save_chart_export("report.txt".into(), vec![1, 2, 3]).is_err());
        assert!(save_chart_export("report.png".into(), vec![1, 2, 3]).is_err());
    }
}
