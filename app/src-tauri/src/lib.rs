mod audio;
mod delivery;
mod dictation;
mod resample;
mod shortcut;
mod storage;
mod transcription;
mod streaming;
mod streaming_core;
mod formatting;
mod learning;
mod notice;

use tauri::{
    menu::{MenuBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

static HIDE_NOTICE_CHECKED: AtomicBool = AtomicBool::new(false);
static MAIN_POSITION_READY: AtomicBool = AtomicBool::new(false);
static MAIN_POSITION: Mutex<Option<(i32, i32)>> = Mutex::new(None);

fn save_main_position(app: &tauri::AppHandle, position: (i32, i32)) {
    let _ = storage::write_setting(app, "main_position", &format!("{},{}", position.0, position.1));
}

fn restore_main_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let saved = storage::read_setting(app, "main_position").ok().flatten()
        .and_then(|value| {
            let (x, y) = value.split_once(',')?;
            Some((x.parse::<i32>().ok()?, y.parse::<i32>().ok()?))
        });
    let restored = saved.and_then(|(x, y)| {
        let size = window.outer_size().ok()?;
        let monitors = window.available_monitors().ok()?;
        let monitor = monitors.iter().min_by_key(|monitor| {
            let area = monitor.work_area();
            let dx = (x as i64 - (area.position.x as i64 + area.size.width as i64 / 2)).abs();
            let dy = (y as i64 - (area.position.y as i64 + area.size.height as i64 / 2)).abs();
            dx + dy
        })?;
        let area = monitor.work_area();
        let max_x = area.position.x.saturating_add(area.size.width as i32 - size.width as i32);
        let max_y = area.position.y.saturating_add(area.size.height as i32 - size.height as i32);
        Some((x.clamp(area.position.x, max_x.max(area.position.x)),
              y.clamp(area.position.y, max_y.max(area.position.y))))
    });
    if let Some((x, y)) = restored {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    } else {
        let _ = window.center();
    }
    MAIN_POSITION_READY.store(true, Ordering::Release);
}

fn show_main(app: &tauri::AppHandle, destination: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(destination) = destination { let _ = app.emit("ui://navigate", destination); }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app, None);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(dictation::DictationState::default())
        .manage(dictation::LastAudioState::default())
        .manage(dictation::LastResultState::default())
        .manage(shortcut::ShortcutStatusState::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                restore_main_position(app.handle(), &window);
            }
            let position_app = app.handle().clone();
            std::thread::spawn(move || {
                let mut saved = None;
                loop {
                    std::thread::sleep(Duration::from_millis(300));
                    let position = MAIN_POSITION.lock().ok().and_then(|value| *value);
                    if let Some(position) = position.filter(|value| Some(*value) != saved) {
                        save_main_position(&position_app, position);
                        saved = Some(position);
                    }
                }
            });
            shortcut::register(app.handle());
            if storage::read_setting(app.handle(), "start_hidden")
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            let help = SubmenuBuilder::new(app, "Ayuda")
                .text("tutorial", "Repetir tutorial completo")
                .text("help", "Cómo dictar y corregir")
                .text("settings", "Atajos y micrófono")
                .build()?;
            let menu = MenuBuilder::new(app)
                .text("show", "Abrir Vozbit")
                .text("toggle", "Iniciar o detener dictado")
                .text("last", "Abrir último texto")
                .text("dictionary", "Abrir diccionario y correcciones")
                .separator()
                .item(&help)
                .separator()
                .text("quit", "Salir por completo")
                .build()?;
            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .ok_or("No hay icono de aplicación")?
                        .clone(),
                )
                .tooltip("Vozbit · dictado local")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main(app, None),
                    "tutorial" => show_main(app, Some("tutorial")),
                    "help" => show_main(app, Some("help")),
                    "settings" => show_main(app, Some("settings")),
                    "toggle" => {
                        let _ = app.emit("shortcut://toggle", ());
                    }
                    "dictionary" => show_main(app, Some("dictionary")),
                    "last" => show_main(app, Some("last")),
                    "quit" => {
                        if let Some(position) = MAIN_POSITION.lock().ok().and_then(|value| *value) {
                            save_main_position(app, position);
                        }
                        app.exit(0)
                    },
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle(), None);
                    }
                })
                .build(app)?;
            dictation::hide_overlay(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::Moved(position) = event {
                    if MAIN_POSITION_READY.load(Ordering::Acquire)
                        && !window.is_maximized().unwrap_or(false)
                        && !window.is_minimized().unwrap_or(false)
                        && window.is_visible().unwrap_or(false) {
                        if let Ok(mut slot) = MAIN_POSITION.lock() { *slot = Some((position.x, position.y)); }
                    }
                }
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if let Ok(position) = window.outer_position() {
                        save_main_position(window.app_handle(), (position.x, position.y));
                    }
                    api.prevent_close(); let _ = window.hide();
                    if !HIDE_NOTICE_CHECKED.swap(true, Ordering::Relaxed) {
                        let app = window.app_handle().clone();
                        std::thread::spawn(move || {
                            if storage::read_setting(&app, "tray_hint_shown").ok().flatten().as_deref() != Some("true") {
                                if storage::write_setting(&app, "tray_hint_shown", "true").is_ok() {
                                notice::show(&app, "Vozbit sigue activo en la bandeja. Clic izquierdo para abrir; clic derecho para Ayuda o Salir por completo.");
                                }
                            }
                        });
                    }
                }
            }
            if window.label() == "overlay" {
                if let tauri::WindowEvent::Moved(position) = event {
                    // Only remember deliberate moves while the indicator is visible.
                    if window.is_visible().unwrap_or(false) {
                        let app = window.app_handle();
                        let _ = storage::write_setting(app,"overlay_x",&position.x.to_string());
                        let _ = storage::write_setting(app,"overlay_y",&position.y.to_string());
                        let _ = storage::write_setting(app,"overlay_custom","true");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            audio::audio_list_input_devices,
            audio::audio_test_input_device,
            transcription::model_status,
            transcription::transcription_transcribe_diagnostic,
            dictation::dictation_start,
            dictation::dictation_stop,
            dictation::dictation_cancel,
            dictation::overlay_drag,
            dictation::dictation_status,
            dictation::dictation_last_result,
            dictation::transcription_cancel,
            dictation::dictation_reprocess,
            storage::history_save,
            storage::history_list,
            storage::history_set_favorite,
            storage::history_delete,
            shortcut::shortcut_status,
            shortcut::shortcut_update,
            storage::dictionary_list,
            storage::dictionary_upsert,
            storage::dictionary_delete,
            storage::dictionary_preview,
            storage::dictionary_suggestions,
            storage::dictionary_review,
            storage::dictionary_set_enabled,
            storage::settings_get,
            storage::settings_patch
        ])
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar Vozbit");
}
