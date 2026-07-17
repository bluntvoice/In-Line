mod database;
mod models;

use database::Database;
use models::*;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};

fn emit_change(app: &tauri::AppHandle) -> Result<(), String> {
    app.emit("data-changed", ())
        .map_err(|error| error.to_string())
}
fn show_main(app: &tauri::AppHandle) {
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
#[tauri::command]
fn get_logs(db: State<Database>, task_id: i64) -> Result<Vec<TaskLog>, String> {
    db.get_logs(task_id)
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
fn add_master(db: State<Database>, kind: String, name: String) -> Result<MasterData, String> {
    db.add_master(kind, name)
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
fn restore_backup(app: tauri::AppHandle, db: State<Database>, path: String) -> Result<(), String> {
    db.restore_backup(path)?;
    emit_change(&app)
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
        "complete" => db.set_status(request.id, "completed".into())?,
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
    show_main(&app);
    app.emit("new-task", ()).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            app.manage(Database::open().map_err(std::io::Error::other)?);
            let open = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
            let add = MenuItem::with_id(app, "add", "新增事项", true, None::<&str>)?;
            let float = MenuItem::with_id(app, "floating", "显示悬浮窗", true, None::<&str>)?;
            let backup = MenuItem::with_id(app, "backup", "立即备份", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&add, &open, &float, &backup, &quit])?;
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "add" => {
                        show_main(app);
                        let _ = app.emit("new-task", ());
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
                .build(app)?;
            show_main(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
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
            get_logs,
            add_log,
            add_master,
            list_backups,
            create_backup,
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
