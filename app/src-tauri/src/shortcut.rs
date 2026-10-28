use crate::storage;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+Space";

struct ShortcutData {
    shortcut: String,
    error: Option<String>,
}

impl Default for ShortcutData {
    fn default() -> Self {
        Self {
            shortcut: DEFAULT_SHORTCUT.into(),
            error: None,
        }
    }
}

#[derive(Default)]
pub struct ShortcutStatusState(Mutex<ShortcutData>);

#[derive(Serialize)]
pub struct ShortcutStatus {
    shortcut: String,
    registered: bool,
    error: Option<String>,
}

pub fn register(app: &AppHandle) {
    let _ = app.global_shortcut().on_shortcut("Ctrl+Alt+L", |app,_,event| {
        if event.state == ShortcutState::Pressed {crate::learning::selected_word(app);}
    });
    let handle = app.clone();
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState,VK_CONTROL,VK_SHIFT,VK_MENU};
        let mut was_down = false;
        loop {
            let enabled = handle.state::<ShortcutStatusState>().0.lock().map(|s|s.shortcut=="Ctrl+Shift").unwrap_or(false);
            let down = enabled && unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0 && unsafe { GetAsyncKeyState(VK_SHIFT as i32) } < 0 && unsafe { GetAsyncKeyState(VK_MENU as i32) } >= 0;
            if down != was_down { let _ = handle.emit("shortcut://state",if down {"pressed"} else {"released"}); }
            was_down=down;
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    });
    let _ = app.global_shortcut().on_shortcut("Ctrl+Alt+Escape", |app,_,event| {
        if event.state == ShortcutState::Pressed { crate::transcription::cancel(); let _=app.emit("shortcut://cancel",()); }
    });
    let _ = app.global_shortcut().on_shortcut("Ctrl+Alt+Space", |app,_,event| {
        if event.state == ShortcutState::Pressed { let _=app.emit("shortcut://reprocess",()); }
    });
    let saved = storage::read_setting(app, "shortcut")
        .ok()
        .flatten()
        .filter(|value| allowed(value))
        .unwrap_or_else(|| DEFAULT_SHORTCUT.into());
    let result = if saved == "Ctrl+Shift" { Ok(()) } else { app
        .global_shortcut()
        .on_shortcut(saved.as_str(), |app, _shortcut, event| {
            let state = if event.state == ShortcutState::Pressed { "pressed" } else { "released" };
            let _ = app.emit("shortcut://state", state);
        }) };
    if let Ok(mut status) = app.state::<ShortcutStatusState>().0.lock() {
        status.shortcut = saved;
        status.error = result
            .err()
            .map(|error| format!("El atajo no está disponible: {error}"));
    }
}

fn allowed(value: &str) -> bool {
    value == "Ctrl+Shift" || (value.len() < 80 && value.contains('+') && !matches!(value,"Ctrl+Alt+Escape"|"Ctrl+Alt+Esc"|"Ctrl+Alt+Space"|"Ctrl+Alt+L") && value.parse::<tauri_plugin_global_shortcut::Shortcut>().is_ok())
}

#[tauri::command]
pub fn shortcut_status(state: State<'_, ShortcutStatusState>) -> Result<ShortcutStatus, String> {
    let current = state
        .0
        .lock()
        .map_err(|_| "No se pudo consultar el estado del atajo".to_string())?;
    Ok(ShortcutStatus {
        shortcut: current.shortcut.clone(),
        registered: current.error.is_none(),
        error: current.error.clone(),
    })
}

#[tauri::command]
pub fn shortcut_update(
    app: AppHandle,
    state: State<'_, ShortcutStatusState>,
    shortcut: String,
) -> Result<ShortcutStatus, String> {
    if !allowed(&shortcut) {
        return Err("Elige una de las combinaciones ofrecidas en Configuración".into());
    }
    let mut current = state
        .0
        .lock()
        .map_err(|_| "No se pudo modificar el atajo".to_string())?;
    if current.shortcut == shortcut && current.error.is_none() {
        return Ok(ShortcutStatus {
            shortcut,
            registered: true,
            error: None,
        });
    }
    if shortcut != "Ctrl+Shift" { app.global_shortcut()
        .on_shortcut(shortcut.as_str(), |app, _shortcut, event| {
            let state = if event.state == ShortcutState::Pressed { "pressed" } else { "released" };
            let _ = app.emit("shortcut://state", state);
        })
        .map_err(|error| format!("Windows no permitió usar ese atajo: {error}"))?; }
    if let Err(error) = storage::write_setting(&app, "shortcut", &shortcut) {
        let _ = app.global_shortcut().unregister(shortcut.as_str());
        return Err(error);
    }
    if current.error.is_none() && current.shortcut != "Ctrl+Shift" {
        if let Err(error) = app.global_shortcut().unregister(current.shortcut.as_str()) {
            let _ = app.global_shortcut().unregister(shortcut.as_str());
            let _ = storage::write_setting(&app, "shortcut", &current.shortcut);
            return Err(format!("No se pudo sustituir el atajo anterior: {error}"));
        }
    }
    current.shortcut = shortcut.clone();
    current.error = None;
    Ok(ShortcutStatus {
        shortcut,
        registered: true,
        error: None,
    })
}
