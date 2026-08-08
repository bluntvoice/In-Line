pub mod database;
pub mod models;

use database::Database;
use models::*;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_window_state::StateFlags;

fn emit_change(app: &tauri::AppHandle) -> Result<(), String> {
    app.emit("data-changed", ())
        .map_err(|error| error.to_string())
}
fn show_main(app: &tauri::AppHandle) {
    for label in ["floating", "quick-add"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
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
}
fn show_quick_add(app: &tauri::AppHandle) {
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
fn archive_task(app: tauri::AppHandle, db: State<Database>, id: i64) -> Result<(), String> {
    db.archive(id)?;
    emit_change(&app)
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
fn get_logs(db: State<Database>, task_id: i64) -> Result<Vec<TaskLog>, String> {
    db.get_logs(task_id)
}
#[tauri::command]
fn get_work_events(db: State<Database>, task_id: i64) -> Result<Vec<TaskWorkEvent>, String> {
    db.list_work_events(task_id)
}
#[tauri::command]
fn record_work_event(
    app: tauri::AppHandle,
    db: State<Database>,
    input: WorkEventInput,
) -> Result<(), String> {
    db.record_work_event(input)?;
    emit_change(&app)
}
#[tauri::command]
fn update_work_event(
    app: tauri::AppHandle,
    db: State<Database>,
    input: WorkEventUpdateInput,
) -> Result<(), String> {
    db.update_work_event(input)?;
    emit_change(&app)
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
fn get_statistics_details(
    db: State<Database>,
    start: String,
    end: String,
    task_type: String,
) -> Result<Vec<StatisticsDetail>, String> {
    db.statistics_details(start, end, task_type)
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
        "请为当前 AI 工具接入以下 In Line MCP 服务。\n\n服务器名称：in_line\n传输方式：stdio\n启动命令：{}\n启动参数：无\n\n可用工具：\n- get_report_summary：读取指定日期范围的统计汇总\n- list_report_items：分页读取指定日期范围的办理事项明细\n\n权限范围：仅只读；不返回联系人、事项详情、内部备注、普通操作日志或回收站事项。\n\n请完成配置、检查格式和程序路径，并告诉我是否需要重启当前 AI 工具。",
        executable.display()
    ))
}
#[tauri::command]
fn delete_backup(app: tauri::AppHandle, db: State<Database>, path: String) -> Result<(), String> {
    db.delete_backup(path)?;
    emit_change(&app)
}
#[tauri::command]
fn set_setting(
    app: tauri::AppHandle,
    db: State<Database>,
    key: String,
    value: String,
) -> Result<(), String> {
    db.set_setting(key, value)?;
    emit_change(&app)
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
        "view" | "edit" | "status" | "urgent" => {
            db.get_task(request.id)?;
            show_main(&app);
            app.emit("task-ui-action", request)
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        "complete" => db.complete_round(request.id)?,
        "archive" => db.archive(request.id)?,
        "delete" => db.soft_delete(request.id)?,
        "restore" => db.restore(request.id)?,
        _ => return Err("不支持的事项操作".into()),
    }
    emit_change(&app)
}
#[tauri::command]
fn toggle_floating(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("floating").ok_or("悬浮窗尚未创建")?;
    let visible = window.is_visible().map_err(|error| error.to_string())?;
    if visible {
        window.hide()
    } else {
        window.show()
    }
    .map_err(|error| error.to_string())?;
    Ok(!visible)
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

pub fn run() {
    let database = Database::open().expect("In Line 数据库初始化失败");
    tauri::Builder::default()
        .manage(database)
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(persisted_window_state_flags())
                .build(),
        )
        .setup(|app| {
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
            show_main(app.handle());
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
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            list_tasks,
            save_task,
            set_task_status,
            move_task,
            delete_task,
            restore_task,
            archive_task,
            merge_tasks,
            resolve_import_conflict,
            get_logs,
            get_work_events,
            record_work_event,
            update_work_event,
            void_work_event,
            process_round,
            complete_round,
            enqueue_task,
            reopen_task,
            get_statistics,
            get_statistics_details,
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
            delete_backup,
            set_setting,
            get_launch_at_login,
            set_launch_at_login,
            restore_backup,
            copy_ticket_card,
            open_task_action,
            toggle_floating,
            show_main_window,
            request_new_task
        ])
        .run(tauri::generate_context!())
        .expect("In Line 启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_visibility_is_not_restored_on_startup() {
        assert!(!persisted_window_state_flags().contains(StateFlags::VISIBLE));
    }
}
