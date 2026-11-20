use crate::storage;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+Space";
pub const DEFAULT_CANCEL_SHORTCUT: &str = "Ctrl+Alt+Escape";
pub const DEFAULT_REPROCESS_SHORTCUT: &str = "Ctrl+Alt+Space";
pub const DEFAULT_LEARN_SHORTCUT: &str = "Ctrl+Alt+L";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    Transcription,
    Cancel,
    Reprocess,
    LearnSelected,
}

const ACTIONS: [ShortcutAction; 4] = [
    ShortcutAction::Transcription,
    ShortcutAction::Cancel,
    ShortcutAction::Reprocess,
    ShortcutAction::LearnSelected,
];

impl ShortcutAction {
    fn storage_key(self) -> &'static str {
        match self {
            Self::Transcription => "shortcut",
            Self::Cancel => "shortcut_cancel",
            Self::Reprocess => "shortcut_reprocess",
            Self::LearnSelected => "shortcut_learn_selected",
        }
    }

    fn default_shortcut(self) -> &'static str {
        match self {
            Self::Transcription => DEFAULT_SHORTCUT,
            Self::Cancel => DEFAULT_CANCEL_SHORTCUT,
            Self::Reprocess => DEFAULT_REPROCESS_SHORTCUT,
            Self::LearnSelected => DEFAULT_LEARN_SHORTCUT,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Transcription => "transcripción",
            Self::Cancel => "cancelar dictado",
            Self::Reprocess => "reprocesar el último audio",
            Self::LearnSelected => "agregar la palabra seleccionada",
        }
    }
}

#[derive(Clone)]
struct ShortcutBinding {
    shortcut: String,
    error: Option<String>,
}

impl ShortcutBinding {
    fn new(shortcut: &str) -> Self {
        Self {
            shortcut: shortcut.into(),
            error: None,
        }
    }

    fn status(&self) -> ShortcutStatus {
        ShortcutStatus {
            shortcut: self.shortcut.clone(),
            registered: self.error.is_none(),
            error: self.error.clone(),
        }
    }
}

struct ShortcutData {
    transcription: ShortcutBinding,
    cancel: ShortcutBinding,
    reprocess: ShortcutBinding,
    learn_selected: ShortcutBinding,
}

impl Default for ShortcutData {
    fn default() -> Self {
        Self {
            transcription: ShortcutBinding::new(DEFAULT_SHORTCUT),
            cancel: ShortcutBinding::new(DEFAULT_CANCEL_SHORTCUT),
            reprocess: ShortcutBinding::new(DEFAULT_REPROCESS_SHORTCUT),
            learn_selected: ShortcutBinding::new(DEFAULT_LEARN_SHORTCUT),
        }
    }
}

impl ShortcutData {
    fn binding(&self, action: ShortcutAction) -> &ShortcutBinding {
        match action {
            ShortcutAction::Transcription => &self.transcription,
            ShortcutAction::Cancel => &self.cancel,
            ShortcutAction::Reprocess => &self.reprocess,
            ShortcutAction::LearnSelected => &self.learn_selected,
        }
    }

    fn binding_mut(&mut self, action: ShortcutAction) -> &mut ShortcutBinding {
        match action {
            ShortcutAction::Transcription => &mut self.transcription,
            ShortcutAction::Cancel => &mut self.cancel,
            ShortcutAction::Reprocess => &mut self.reprocess,
            ShortcutAction::LearnSelected => &mut self.learn_selected,
        }
    }

    fn statuses(&self) -> ShortcutBindings {
        ShortcutBindings {
            transcription: self.transcription.status(),
            cancel: self.cancel.status(),
            reprocess: self.reprocess.status(),
            learn_selected: self.learn_selected.status(),
        }
    }
}

#[derive(Default)]
pub struct ShortcutStatusState(Mutex<ShortcutData>);

#[derive(Clone, Serialize)]
pub struct ShortcutStatus {
    shortcut: String,
    registered: bool,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct ShortcutBindings {
    transcription: ShortcutStatus,
    cancel: ShortcutStatus,
    reprocess: ShortcutStatus,
    learn_selected: ShortcutStatus,
}

fn canonical(value: &str) -> String {
    value
        .split('+')
        .map(str::trim)
        .map(|part| {
            if part.eq_ignore_ascii_case("esc") {
                "escape"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join("+")
        .to_ascii_lowercase()
}

fn allowed(action: ShortcutAction, value: &str) -> bool {
    (action == ShortcutAction::Transcription && value == "Ctrl+Shift")
        || (value.len() < 80
            && value.contains('+')
            && value
                .parse::<tauri_plugin_global_shortcut::Shortcut>()
                .is_ok())
}

fn dispatch(app: &AppHandle, action: ShortcutAction, state: ShortcutState) {
    match action {
        ShortcutAction::Transcription => {
            let _ = app.emit(
                "shortcut://state",
                if state == ShortcutState::Pressed {
                    "pressed"
                } else {
                    "released"
                },
            );
        }
        ShortcutAction::Cancel if state == ShortcutState::Pressed => {
            crate::dictation::cancel_processing(app);
            let _ = app.emit("shortcut://cancel", ());
        }
        ShortcutAction::Reprocess if state == ShortcutState::Pressed => {
            let _ = app.emit("shortcut://reprocess", ());
        }
        ShortcutAction::LearnSelected if state == ShortcutState::Pressed => {
            crate::learning::selected_word(app);
        }
        _ => {}
    }
}

fn register_action(app: &AppHandle, action: ShortcutAction, shortcut: &str) -> Result<(), String> {
    if action == ShortcutAction::Transcription && shortcut == "Ctrl+Shift" {
        return Ok(());
    }
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _, event| {
            dispatch(app, action, event.state);
        })
        .map_err(|error| format!("Windows no permitió usar ese atajo: {error}"))
}

fn load_bindings(app: &AppHandle) -> ShortcutData {
    let mut data = ShortcutData::default();
    for action in ACTIONS {
        let saved = storage::read_setting(app, action.storage_key())
            .ok()
            .flatten()
            .filter(|value| allowed(action, value))
            .unwrap_or_else(|| action.default_shortcut().into());
        data.binding_mut(action).shortcut = saved;
    }
    data
}

pub fn register(app: &AppHandle) {
    let mut data = load_bindings(app);
    for (index, action) in ACTIONS.into_iter().enumerate() {
        let shortcut = data.binding(action).shortcut.clone();
        let duplicate = ACTIONS[..index]
            .iter()
            .any(|other| canonical(&data.binding(*other).shortcut) == canonical(&shortcut));
        let result = if duplicate {
            Err("La combinación está repetida en otro atajo".into())
        } else {
            register_action(app, action, &shortcut)
        };
        data.binding_mut(action).error = result.err();
    }
    if let Ok(mut current) = app.state::<ShortcutStatusState>().0.lock() {
        *current = data;
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
        };
        let mut was_down = false;
        loop {
            let enabled = handle
                .state::<ShortcutStatusState>()
                .0
                .lock()
                .map(|state| state.transcription.shortcut == "Ctrl+Shift")
                .unwrap_or(false);
            let down = enabled
                && unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0
                && unsafe { GetAsyncKeyState(VK_SHIFT as i32) } < 0
                && unsafe { GetAsyncKeyState(VK_MENU as i32) } >= 0;
            if down != was_down {
                let _ = handle.emit(
                    "shortcut://state",
                    if down { "pressed" } else { "released" },
                );
            }
            was_down = down;
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    });
}

#[tauri::command]
pub fn shortcut_status(state: State<'_, ShortcutStatusState>) -> Result<ShortcutStatus, String> {
    let current = state
        .0
        .lock()
        .map_err(|_| "No se pudo consultar el estado del atajo".to_string())?;
    Ok(current.transcription.status())
}

#[tauri::command]
pub fn shortcuts_status(state: State<'_, ShortcutStatusState>) -> Result<ShortcutBindings, String> {
    let current = state
        .0
        .lock()
        .map_err(|_| "No se pudieron consultar los atajos".to_string())?;
    Ok(current.statuses())
}

fn update_action(
    app: &AppHandle,
    state: &State<'_, ShortcutStatusState>,
    action: ShortcutAction,
    shortcut: String,
) -> Result<ShortcutBindings, String> {
    let shortcut = shortcut.trim().to_string();
    if !allowed(action, &shortcut) {
        return Err("Pulsa una combinación con al menos una tecla modificadora".into());
    }
    let mut current = state
        .0
        .lock()
        .map_err(|_| "No se pudo modificar el atajo".to_string())?;
    if let Some(other) = ACTIONS.into_iter().find(|other| {
        *other != action && canonical(&current.binding(*other).shortcut) == canonical(&shortcut)
    }) {
        return Err(format!("Esa combinación ya se usa para {}", other.label()));
    }
    let previous = current.binding(action).clone();
    if canonical(&previous.shortcut) == canonical(&shortcut) && previous.error.is_none() {
        return Ok(current.statuses());
    }
    register_action(app, action, &shortcut)?;
    if let Err(error) = storage::write_setting(app, action.storage_key(), &shortcut) {
        if !(action == ShortcutAction::Transcription && shortcut == "Ctrl+Shift") {
            let _ = app.global_shortcut().unregister(shortcut.as_str());
        }
        return Err(error);
    }
    if previous.error.is_none()
        && !(action == ShortcutAction::Transcription && previous.shortcut == "Ctrl+Shift")
    {
        if let Err(error) = app.global_shortcut().unregister(previous.shortcut.as_str()) {
            if !(action == ShortcutAction::Transcription && shortcut == "Ctrl+Shift") {
                let _ = app.global_shortcut().unregister(shortcut.as_str());
            }
            let _ = storage::write_setting(app, action.storage_key(), &previous.shortcut);
            return Err(format!("No se pudo sustituir el atajo anterior: {error}"));
        }
    }
    let binding = current.binding_mut(action);
    binding.shortcut = shortcut;
    binding.error = None;
    Ok(current.statuses())
}

#[tauri::command]
pub fn shortcut_update(
    app: AppHandle,
    state: State<'_, ShortcutStatusState>,
    shortcut: String,
) -> Result<ShortcutStatus, String> {
    let statuses = update_action(&app, &state, ShortcutAction::Transcription, shortcut)?;
    Ok(statuses.transcription)
}

#[tauri::command]
pub fn shortcut_action_update(
    app: AppHandle,
    state: State<'_, ShortcutStatusState>,
    action: ShortcutAction,
    shortcut: String,
) -> Result<ShortcutBindings, String> {
    update_action(&app, &state, action, shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unique_and_valid() {
        for action in ACTIONS {
            assert!(allowed(action, action.default_shortcut()));
            assert_eq!(
                ACTIONS
                    .into_iter()
                    .filter(|other| {
                        canonical(other.default_shortcut()) == canonical(action.default_shortcut())
                    })
                    .count(),
                1
            );
        }
    }

    #[test]
    fn modifier_only_shortcut_is_limited_to_transcription() {
        assert!(allowed(ShortcutAction::Transcription, "Ctrl+Shift"));
        assert!(!allowed(ShortcutAction::Cancel, "Ctrl+Shift"));
        assert!(!allowed(ShortcutAction::Reprocess, "Escape"));
    }

    #[test]
    fn escape_aliases_compare_equally() {
        assert_eq!(canonical("Ctrl+Alt+Esc"), canonical("Ctrl + Alt + Escape"));
    }
}
