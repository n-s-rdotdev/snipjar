mod storage;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::ActivationPolicy;
use tauri::Manager;
use tauri::State;
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const MAIN_WINDOW_LABEL: &str = "main";
const RELEASE_NOTES_WINDOW_LABEL: &str = "release-notes";
const TRAY_ICON_ID: &str = "snipjar-menu-bar";
const TRAY_SHOW_MENU_ID: &str = "tray-show";
const TRAY_RELEASE_NOTES_MENU_ID: &str = "tray-release-notes";
const TRAY_QUIT_MENU_ID: &str = "tray-quit";

#[derive(Default)]
struct RuntimeState {
    is_quitting: AtomicBool,
    previous_app_bundle_id: Mutex<Option<String>>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn create_entry(
    state: State<'_, storage::DatabaseState>,
    input: storage::EntryInput,
) -> Result<storage::Entry, String> {
    storage::create_entry(state.inner(), input).map_err(|err| err.to_string())
}

#[tauri::command]
fn update_entry(
    state: State<'_, storage::DatabaseState>,
    id: String,
    input: storage::EntryInput,
) -> Result<storage::Entry, String> {
    storage::update_entry(state.inner(), &id, input).map_err(|err| err.to_string())
}

#[tauri::command]
fn get_entry(
    state: State<'_, storage::DatabaseState>,
    id: String,
) -> Result<storage::Entry, String> {
    storage::get_entry(state.inner(), &id).map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_entry(state: State<'_, storage::DatabaseState>, id: String) -> Result<(), String> {
    storage::delete_entry(state.inner(), &id).map_err(|err| err.to_string())
}

#[tauri::command]
fn get_recent_entries(
    state: State<'_, storage::DatabaseState>,
) -> Result<Vec<storage::EntrySummary>, String> {
    storage::get_recent_entries(state.inner()).map_err(|err| err.to_string())
}

#[tauri::command]
fn search_entries(
    state: State<'_, storage::DatabaseState>,
    query: String,
) -> Result<Vec<storage::EntrySummary>, String> {
    storage::search_entries(state.inner(), &query).map_err(|err| err.to_string())
}

#[tauri::command]
fn copy_entry(
    app_handle: tauri::AppHandle,
    state: State<'_, storage::DatabaseState>,
    id: String,
) -> Result<storage::PasteResult, String> {
    let result = storage::copy_entry(state.inner(), &id).map_err(|err| err.to_string())?;
    hide_launcher_window(&app_handle)
        .map_err(|err| format!("failed to hide launcher after copy: {err}"))?;
    let _ = restore_previous_application(&app_handle);
    Ok(result)
}

#[tauri::command]
fn paste_entry(
    app_handle: tauri::AppHandle,
    state: State<'_, storage::DatabaseState>,
    id: String,
) -> Result<storage::PasteResult, String> {
    hide_launcher_window(&app_handle)
        .map_err(|err| format!("failed to hide launcher before paste: {err}"))?;
    let _ = restore_previous_application(&app_handle);
    thread::sleep(Duration::from_millis(120));

    let result = storage::paste_entry(state.inner(), &id).map_err(|err| err.to_string())?;
    if matches!(result.mode, storage::PasteMode::CopiedOnly) {
        let _ = show_launcher_window(&app_handle);
    }

    Ok(result)
}

fn hide_launcher_window(app_handle: &tauri::AppHandle) -> Result<(), tauri::Error> {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        if window.is_visible()? {
            window.hide()?;
        }
    }

    Ok(())
}

fn show_launcher_window(app_handle: &tauri::AppHandle) -> Result<(), tauri::Error> {
    remember_previous_application(app_handle);

    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
    }

    Ok(())
}

fn remember_previous_application(app_handle: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    if let Some(bundle_id) = current_frontmost_application_bundle_id() {
        if bundle_id != app_handle.config().identifier {
            if let Ok(mut previous_app_bundle_id) = app_handle
                .state::<RuntimeState>()
                .previous_app_bundle_id
                .lock()
            {
                *previous_app_bundle_id = Some(bundle_id);
            }
        }
    }
}

fn restore_previous_application(app_handle: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let previous_app_bundle_id = app_handle
            .state::<RuntimeState>()
            .previous_app_bundle_id
            .lock()
            .map_err(|_| "failed to access previous app state".to_string())?
            .clone();

        if let Some(bundle_id) = previous_app_bundle_id {
            if bundle_id != app_handle.config().identifier {
                activate_application(&bundle_id)?;
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn current_frontmost_application_bundle_id() -> Option<String> {
    run_osascript("id of application (path to frontmost application as text)")
        .ok()
        .filter(|bundle_id| !bundle_id.is_empty())
}

#[cfg(target_os = "macos")]
fn activate_application(bundle_id: &str) -> Result<(), String> {
    run_osascript(&format!("tell application id \"{bundle_id}\" to activate")).map(|_| ())
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("osascript failed with status {}", output.status)
        } else {
            stderr
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn toggle_launcher_window(app_handle: &tauri::AppHandle) -> Result<(), tauri::Error> {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        if window.is_visible()? && window.is_focused()? {
            hide_launcher_window(app_handle)?;
        } else {
            show_launcher_window(app_handle)?;
        }
    }

    Ok(())
}

fn request_app_quit(app_handle: &tauri::AppHandle) {
    let runtime_state = app_handle.state::<RuntimeState>();
    runtime_state.is_quitting.store(true, Ordering::Relaxed);
    app_handle.exit(0);
}

fn open_release_notes_window(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window(RELEASE_NOTES_WINDOW_LABEL) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        app_handle,
        RELEASE_NOTES_WINDOW_LABEL,
        tauri::WebviewUrl::App("release-notes.html".into()),
    )
    .title("Release notes")
    .inner_size(540.0, 640.0)
    .min_inner_size(420.0, 520.0)
    .resizable(true)
    .maximizable(false)
    .build()?;

    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::with_id(TRAY_SHOW_MENU_ID, "Show").build(app)?;
    let release_notes_item = MenuItemBuilder::with_id(TRAY_RELEASE_NOTES_MENU_ID, "Release notes")
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id(TRAY_QUIT_MENU_ID, "Quit")
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;
    let tray_menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&release_notes_item)
        .item(&quit_item)
        .build()?;
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
        .expect("failed to load tray icon");

    TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&tray_menu)
        .icon(tray_icon)
        .icon_as_template(true)
        .show_menu_on_left_click(true)
        .on_menu_event(|app_handle, event| {
            if event.id() == TRAY_SHOW_MENU_ID {
                let _ = show_launcher_window(app_handle);
            } else if event.id() == TRAY_RELEASE_NOTES_MENU_ID {
                let _ = open_release_notes_window(app_handle);
            } else if event.id() == TRAY_QUIT_MENU_ID {
                request_app_quit(app_handle);
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app_handle, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_launcher_window(app_handle);
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            let runtime_state = window.state::<RuntimeState>();
            if runtime_state.is_quitting.load(Ordering::Relaxed) {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let db_state = storage::initialize(app.handle())?;
            #[cfg(debug_assertions)]
            let db_path = db_state.path.clone();
            app.manage(db_state);
            app.manage(RuntimeState::default());
            #[cfg(target_os = "macos")]
            app.handle().set_activation_policy(ActivationPolicy::Accessory)?;
            setup_tray(app)?;
            let launcher_shortcut =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
            app.global_shortcut().register(launcher_shortcut)?;
            #[cfg(debug_assertions)]
            eprintln!("snipjar sqlite initialized at {}", db_path.display());
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            create_entry,
            update_entry,
            get_entry,
            delete_entry,
            get_recent_entries,
            search_entries,
            copy_entry,
            paste_entry
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let runtime_state = app_handle.state::<RuntimeState>();
                runtime_state.is_quitting.store(true, Ordering::Relaxed);
            }
        });
}
