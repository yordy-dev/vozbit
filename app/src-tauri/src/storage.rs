use regex::{Captures, RegexBuilder};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

static SCHEMA_READY: Mutex<bool> = Mutex::new(false);

#[derive(Serialize)]
pub struct HistoryItem {
    id: i64,
    created_at_ms: i64,
    text: String,
    language: String,
    audio_seconds: f64,
    favorite: bool,
}

#[derive(Serialize)]
pub struct DictionaryEntry {
    term: String,
    replacement: String,
    source: String,
    enabled: bool,
}

#[derive(Serialize)]
pub struct DictionaryPreview {
    pub(crate) text: String,
    pub(crate) changes: usize,
}

#[derive(Serialize)]
pub struct AppSettings {
    tutorial_version: u8,
    learn_corrections: bool,
    language: String,
    transcription_engine: String,
    preferred_device_id: String,
    auto_insert: bool,
    auto_save: bool,
    append_trailing_space: bool,
    text_correction: bool,
    format_numbers: bool,
    math_symbols: bool,
    push_to_talk: bool,
    overlay_position: String,
    overlay_opacity: u8,
    start_hidden: bool,
    launch_at_startup: bool,
    history_limit: i64,
}

#[derive(Deserialize)]
pub struct SettingsPatch {
    tutorial_version: Option<u8>,
    learn_corrections: Option<bool>,
    language: Option<String>,
    transcription_engine: Option<String>,
    preferred_device_id: Option<String>,
    auto_insert: Option<bool>,
    auto_save: Option<bool>,
    append_trailing_space: Option<bool>,
    text_correction: Option<bool>,
    format_numbers: Option<bool>,
    math_symbols: Option<bool>,
    push_to_talk: Option<bool>,
    overlay_position: Option<String>,
    overlay_opacity: Option<u8>,
    start_hidden: Option<bool>,
    launch_at_startup: Option<bool>,
    history_limit: Option<i64>,
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(folder) = std::env::var_os("VOICETEXT_DATA_DIR") {
        let path = PathBuf::from(folder);
        if !path.is_absolute() {
            return Err("La carpeta aislada de datos debe ser absoluta".into());
        }
        return Ok(path.join("voicetext.sqlite3"));
    }
    app.path()
        .app_data_dir()
        .map(|folder| folder.join("voicetext.sqlite3"))
        .map_err(|error| format!("No se encontró la carpeta de datos de la aplicación: {error}"))
}

fn open(app: &AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    fs::create_dir_all(path.parent().ok_or("Ruta de datos inválida")?)
        .map_err(|error| format!("No se pudo crear la carpeta de datos: {error}"))?;
    let connection = Connection::open(&path)
        .map_err(|error| format!("No se pudo abrir la base de datos local: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(|error| error.to_string())?;
    let mut schema_ready = SCHEMA_READY
        .lock()
        .map_err(|_| "No se pudo preparar la base local".to_string())?;
    if !*schema_ready {
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
             BEGIN;
             CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY,
                created_at_ms INTEGER NOT NULL,
                text TEXT NOT NULL,
                language TEXT NOT NULL,
                audio_seconds REAL NOT NULL,
                favorite INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS history_created ON history(created_at_ms DESC);
             CREATE TABLE IF NOT EXISTS dictionary (
                term TEXT PRIMARY KEY COLLATE NOCASE,
                replacement TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS dictionary_meta(term TEXT PRIMARY KEY COLLATE NOCASE,source TEXT NOT NULL DEFAULT 'manual',enabled INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE IF NOT EXISTS suggestions(term TEXT PRIMARY KEY COLLATE NOCASE,replacement TEXT NOT NULL);
             PRAGMA user_version=3;
             COMMIT;",
            )
            .map_err(|error| format!("No se pudo preparar la base local: {error}"))?;
        *schema_ready = true;
    }
    Ok(connection)
}

pub(crate) fn read_setting(app: &AppHandle, key: &str) -> Result<Option<String>, String> {
    open(app)?
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| format!("No se pudo leer la configuración local: {error}"))
}

pub(crate) fn write_setting(app: &AppHandle, key: &str, value: &str) -> Result<(), String> {
    open(app)?
        .execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])
        .map_err(|error| format!("No se pudo guardar la configuración local: {error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Result<AppSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_autostart::ManagerExt;
        Ok(AppSettings {
            tutorial_version: read_setting(&app, "tutorial_version")?
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0),
            learn_corrections: read_setting(&app, "learn_corrections")?.as_deref() == Some("true")
                && read_setting(&app, "learning_consent_v048")?.as_deref() == Some("true"),
            language: read_setting(&app, "language")?.unwrap_or_else(|| "es".into()),
            transcription_engine: read_setting(&app, "transcription_engine")?
                .filter(|value| matches!(value.as_str(), "whisper" | "parakeet_ultra"))
                .unwrap_or_else(|| "whisper".into()),
            preferred_device_id: read_setting(&app, "preferred_device_id")?.unwrap_or_default(),
            auto_insert: read_setting(&app, "auto_insert")?.as_deref() != Some("false"),
            auto_save: read_setting(&app, "auto_save")?.as_deref() != Some("false"),
            append_trailing_space: read_setting(&app, "append_trailing_space")?.as_deref() != Some("false"),
            text_correction: read_setting(&app, "text_correction")?.as_deref() != Some("false"),
            format_numbers: read_setting(&app, "format_numbers")?.as_deref() != Some("false"),
            math_symbols: read_setting(&app, "math_symbols")?.as_deref() != Some("false"),
            push_to_talk: read_setting(&app, "push_to_talk")?.as_deref() == Some("true"),
            overlay_position: read_setting(&app, "overlay_position")?
                .filter(|value| matches!(value.as_str(), "top_center" | "center" | "bottom_center"))
                .unwrap_or_else(|| "center".into()),
            overlay_opacity: read_setting(&app, "overlay_opacity")?
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|value| (40..=100).contains(value))
                .unwrap_or(100),
            start_hidden: read_setting(&app, "start_hidden")?.as_deref() == Some("true"),
            launch_at_startup: app.autolaunch().is_enabled().unwrap_or(false),
            history_limit: read_setting(&app, "history_limit")?
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| (5..=500).contains(value))
                .unwrap_or(20),
        })
    })
    .await
    .map_err(|error| format!("La lectura de configuración terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn settings_patch(app: AppHandle, patch: SettingsPatch) -> Result<(), String> {
    if patch.tutorial_version.is_some_and(|version| version > 1) {
        return Err("La versión del tutorial no es válida".into());
    }
    if let Some(language) = &patch.language {
        if !matches!(language.as_str(), "auto" | "es" | "en") {
            return Err("El idioma seleccionado no es válido".into());
        }
    }
    if let Some(engine) = &patch.transcription_engine {
        if !matches!(engine.as_str(), "whisper" | "parakeet_ultra") {
            return Err("El motor de transcripción seleccionado no es válido".into());
        }
    }
    if let Some(device) = &patch.preferred_device_id {
        if device.len() > 1000 || device.chars().any(char::is_control) {
            return Err("El identificador del micrófono no es válido".into());
        }
    }
    if let Some(position) = &patch.overlay_position {
        if !matches!(position.as_str(), "top_center" | "center" | "bottom_center") {
            return Err("La posición del indicador no es válida".into());
        }
    }
    if patch.overlay_opacity.is_some_and(|value| !(40..=100).contains(&value)) {
        return Err("La transparencia del indicador debe estar entre 40 % y 100 %".into());
    }
    if let Some(limit) = patch.history_limit {
        if !(5..=500).contains(&limit) {
            return Err("El límite del historial debe estar entre 5 y 500".into());
        }
    }
    if let Some(enabled) = patch.launch_at_startup {
        use tauri_plugin_autostart::ManagerExt;
        if enabled {
            app.autolaunch().enable().map_err(|error| format!("No se pudo activar el inicio con Windows: {error}"))?;
        } else {
            app.autolaunch().disable().map_err(|error| format!("No se pudo desactivar el inicio con Windows: {error}"))?;
        }
    }
    let opacity_change = patch.overlay_opacity;
    let refresh_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if patch.transcription_engine.as_deref() == Some("parakeet_ultra") {
            crate::transcription::verify_parakeet_resources(&app)?;
        }
        if let Some(version) = patch.tutorial_version { write_setting(&app, "tutorial_version", &version.to_string())?; }
        if let Some(enabled) = patch.learn_corrections {
            write_setting(&app, "learn_corrections", if enabled {"true"} else {"false"})?;
            if enabled { write_setting(&app, "learning_consent_v048", "true")?; }
            else { crate::learning::stop(); }
        }
        if patch.overlay_position.is_some() { write_setting(&app, "overlay_custom", "false")?; }
        if let Some(language) = patch.language {
            write_setting(&app, "language", &language)?;
        }
        if let Some(engine) = patch.transcription_engine {
            write_setting(&app, "transcription_engine", &engine)?;
        }
        if let Some(device) = patch.preferred_device_id {
            write_setting(&app, "preferred_device_id", &device)?;
        }
        if let Some(enabled) = patch.auto_insert {
            write_setting(&app, "auto_insert", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.auto_save {
            write_setting(&app, "auto_save", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.append_trailing_space {
            write_setting(&app, "append_trailing_space", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.text_correction {
            write_setting(&app, "text_correction", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.format_numbers {
            write_setting(&app, "format_numbers", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.math_symbols {
            write_setting(&app, "math_symbols", if enabled { "true" } else { "false" })?;
        }
        if let Some(enabled) = patch.push_to_talk {
            write_setting(&app, "push_to_talk", if enabled { "true" } else { "false" })?;
        }
        if let Some(position) = patch.overlay_position {
            write_setting(&app, "overlay_position", &position)?;
        }
        if let Some(value) = patch.overlay_opacity {
            write_setting(&app, "overlay_opacity", &value.to_string())?;
        }
        if let Some(enabled) = patch.start_hidden {
            write_setting(&app, "start_hidden", if enabled { "true" } else { "false" })?;
        }
        if let Some(limit) = patch.history_limit {
            write_setting(&app, "history_limit", &limit.to_string())?;
            open(&app)?
                .execute(
                    "DELETE FROM history WHERE favorite=0 AND id NOT IN (SELECT id FROM history WHERE favorite=0 ORDER BY created_at_ms DESC,id DESC LIMIT ?1)",
                    [limit],
                )
                .map_err(|error| format!("No se pudo ajustar el historial: {error}"))?;
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("El guardado de configuración terminó inesperadamente: {error}"))??;
    if let Some(value) = opacity_change {
        use tauri::Emitter;
        let _ = refresh_app.emit("overlay://opacity", value);
    }
    Ok(())
}

#[tauri::command]
pub async fn history_save(
    app: AppHandle,
    text: String,
    language: String,
    audio_seconds: f64,
) -> Result<i64, String> {
    if text.trim().is_empty() || text.len() > 100_000 {
        return Err("El texto debe tener entre 1 y 100 000 caracteres".into());
    }
    if !matches!(language.as_str(), "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    if !audio_seconds.is_finite() || !(1.0..=120.0).contains(&audio_seconds) {
        return Err("La duración del audio no es válida".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut connection = open(&app)?;
        let limit = read_setting(&app, "history_limit")?
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| (5..=500).contains(value))
            .unwrap_or(20);
        let transaction = connection.transaction()
            .map_err(|error| format!("No se pudo iniciar el guardado del historial: {error}"))?;
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis() as i64;
        transaction
            .execute(
                "INSERT INTO history(created_at_ms,text,language,audio_seconds) VALUES (?1,?2,?3,?4)",
                params![created_at_ms, text.trim(), language, audio_seconds],
            )
            .map_err(|error| format!("No se pudo guardar la transcripción: {error}"))?;
        let inserted_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "DELETE FROM history WHERE favorite=0 AND id NOT IN (SELECT id FROM history WHERE favorite=0 ORDER BY created_at_ms DESC,id DESC LIMIT ?1)",
                [limit],
            )
            .map_err(|error| format!("No se pudo limitar el historial: {error}"))?;
        transaction.commit().map_err(|error| format!("No se pudo confirmar el historial: {error}"))?;
        Ok(inserted_id)
    })
    .await
    .map_err(|error| format!("El guardado terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn history_list(app: AppHandle, query: String) -> Result<Vec<HistoryItem>, String> {
    if query.len() > 200 {
        return Err("La búsqueda es demasiado larga".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let connection = open(&app)?;
        let mut statement = connection
            .prepare("SELECT id,created_at_ms,text,language,audio_seconds,favorite FROM history WHERE instr(lower(text),lower(?1))>0 ORDER BY created_at_ms DESC,id DESC LIMIT 500")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([query], |row| {
                Ok(HistoryItem {
                    id: row.get(0)?,
                    created_at_ms: row.get(1)?,
                    text: row.get(2)?,
                    language: row.get(3)?,
                    audio_seconds: row.get(4)?,
                    favorite: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("No se pudo leer el historial: {error}"))
    })
    .await
    .map_err(|error| format!("La búsqueda terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn history_set_favorite(app: AppHandle, id: i64, favorite: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let connection = open(&app)?;
        let count = connection
            .execute(
                "UPDATE history SET favorite=?1 WHERE id=?2",
                params![favorite, id],
            )
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("La transcripción ya no existe".into());
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("El cambio terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn history_delete(app: AppHandle, id: i64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let connection = open(&app)?;
        let count = connection
            .execute("DELETE FROM history WHERE id=?1", [id])
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("La transcripción ya no existe".into());
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("El borrado terminó inesperadamente: {error}"))?
}

fn read_dictionary(connection: &Connection) -> Result<Vec<DictionaryEntry>, String> {
    let mut statement = connection
        .prepare("SELECT d.term,d.replacement,coalesce(m.source,'manual'),coalesce(m.enabled,1) FROM dictionary d LEFT JOIN dictionary_meta m ON m.term=d.term ORDER BY d.term COLLATE NOCASE LIMIT 500")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(DictionaryEntry {
                term: row.get(0)?,
                replacement: row.get(1)?,
                source: row.get(2)?,
                enabled: row.get::<_,i64>(3)? != 0,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn dictionary_prompt_terms(app: &AppHandle) -> Result<Vec<String>, String> {
    let connection = open(app)?;
    let entries = read_dictionary(&connection)?;
    let mut terms = Vec::new();
    let mut length = 0_usize;
    for entry in entries {
        if !entry.enabled { continue; }
        let candidate = entry.replacement.trim();
        if !is_vocabulary_prompt_term(candidate) || length + candidate.len() > 500 {
            continue;
        }
        length += candidate.len();
        terms.push(candidate.to_string());
    }
    Ok(terms)
}

fn is_vocabulary_prompt_term(term: &str) -> bool {
    !term.is_empty() && term.chars().count() <= 40
        && term.chars().all(|character| character.is_alphabetic() || character.is_whitespace() || character == '-')
}

#[tauri::command]
pub async fn dictionary_list(app: AppHandle) -> Result<Vec<DictionaryEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || read_dictionary(&open(&app)?))
        .await
        .map_err(|error| format!("La lectura del diccionario terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn dictionary_upsert(
    app: AppHandle,
    term: String,
    replacement: String,
) -> Result<(), String> {
    let term = term.trim().to_string();
    let replacement = replacement.trim().to_string();
    if term.is_empty()
        || replacement.is_empty()
        || term.chars().count() > 80
        || replacement.chars().count() > 80
        || term.chars().any(char::is_control)
        || replacement.chars().any(char::is_control)
    {
        return Err("Los campos admiten entre 1 y 80 caracteres sin saltos de línea".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let connection = open(&app)?;
        let count: i64 = connection
            .query_row("SELECT count(*) FROM dictionary", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        let exists: i64 = connection
            .query_row("SELECT count(*) FROM dictionary WHERE term=?1", [&term], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if count >= 500 && exists == 0 {
            return Err("El diccionario alcanzó el límite de 500 términos".into());
        }
        connection
            .execute("INSERT INTO dictionary(term,replacement) VALUES(?1,?2) ON CONFLICT(term) DO UPDATE SET replacement=excluded.replacement", params![term, replacement])
            .map_err(|error| format!("No se pudo guardar el término: {error}"))?;
        connection.execute("INSERT INTO dictionary_meta(term,source,enabled) VALUES(?1,'manual',1) ON CONFLICT(term) DO UPDATE SET source='manual',enabled=1",[&term]).map_err(|error| format!("No se pudo activar el término: {error}"))?;
        Ok(())
    })
    .await
    .map_err(|error| format!("El guardado del término terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn dictionary_delete(app: AppHandle, term: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let connection = open(&app)?;
        let count = connection
            .execute("DELETE FROM dictionary WHERE term=?1", [&term])
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("El término ya no existe".into());
        }
        connection.execute("DELETE FROM dictionary_meta WHERE term=?1",[&term]).map_err(|e|e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|error| format!("El borrado del término terminó inesperadamente: {error}"))?
}

fn apply_entries(entries: &[DictionaryEntry], text: &str) -> Result<DictionaryPreview, String> {
    if entries.is_empty() {
        return Ok(DictionaryPreview {
            text: text.to_string(),
            changes: 0,
        });
    }
    let mut ordered = entries.iter().filter(|e|e.enabled).collect::<Vec<_>>();
    if ordered.is_empty() { return Ok(DictionaryPreview{text:text.to_string(),changes:0}); }
    ordered.sort_by_key(|entry| std::cmp::Reverse(entry.term.chars().count()));
    let terms = ordered
        .into_iter()
        .map(|entry| regex::escape(&entry.term))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = RegexBuilder::new(&format!(r"(?:{terms})"))
        .case_insensitive(true)
        .build()
        .map_err(|_| "No se pudo preparar el diccionario".to_string())?;
    let replacements: HashMap<String, String> = entries
        .iter()
        .map(|entry| (entry.term.to_lowercase(), entry.replacement.clone()))
        .collect();
    let mut changes = 0;
    let updated = pattern.replace_all(text, |captures: &Captures<'_>| {
        let found = captures.get(0).unwrap();
        let left = text[..found.start()].chars().next_back();
        let right = text[found.end()..].chars().next();
        if left.is_some_and(|c| c.is_alphanumeric() || c=='_') || right.is_some_and(|c| c.is_alphanumeric() || c=='_') { return found.as_str().to_string(); }
        let replacement = replacements
            .get(&captures[0].to_lowercase())
            .cloned()
            .unwrap_or_else(|| captures[0].to_string());
        if replacement != found.as_str() { changes += 1; }
        replacement
    });
    Ok(DictionaryPreview {
        text: updated.into_owned(),
        changes,
    })
}

pub(crate) fn apply_dictionary(app: &AppHandle, text: &str) -> Result<DictionaryPreview, String> {
    let connection = open(app)?;
    let entries = read_dictionary(&connection)?;
    apply_entries(&entries, text)
}

fn clear_single_word_correction(term:&str,replacement:&str)->bool {
    let old_len = term.chars().count();
    let new_len = replacement.chars().count();
    if !(3..=32).contains(&old_len) || !(3..=32).contains(&new_len)
        || !term.chars().all(char::is_alphabetic)
        || !replacement.chars().all(char::is_alphabetic)
        || term == replacement { return false; }
    if replacement.chars().all(|character| !character.is_lowercase())
        || replacement.chars().skip(1).any(char::is_uppercase) { return true; }

    // Only an edit of the freshly dictated word can reach this function. Do not
    // turn short words or ordinary singular/plural edits into global rules.
    if old_len < 6 || new_len < 6 { return false; }
    let old = term.to_lowercase();
    let new = replacement.to_lowercase();
    let shared_prefix = old.chars().zip(new.chars()).take_while(|(a,b)| a == b).count();
    if old == new || shared_prefix < 3 || old.starts_with(&new) || new.starts_with(&old)
        || (old_len == new_len && shared_prefix >= old_len.saturating_sub(2)) { return false; }
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let mut previous: Vec<usize> = (0..=new_chars.len()).collect();
    for (row, old_char) in old_chars.iter().enumerate() {
        let mut current = vec![row + 1; new_chars.len() + 1];
        for (column, new_char) in new_chars.iter().enumerate() {
            current[column + 1] = (previous[column + 1] + 1)
                .min(current[column] + 1)
                .min(previous[column] + usize::from(old_char != new_char));
        }
        previous = current;
    }
    let distance = previous[new_chars.len()];
    distance <= 4 && distance * 2 <= old_len.max(new_len)
}

fn reviewable_correction(term: &str, replacement: &str) -> bool {
    let term = term.trim();
    let replacement = replacement.trim();
    if term.is_empty() || replacement.is_empty() || term == replacement
        || term.chars().count() > 80 || replacement.chars().count() > 80
        || term.chars().any(char::is_control) || replacement.chars().any(char::is_control)
        || [term, replacement].iter().any(|value| {
            value.contains('@') || value.contains('\\') || value.contains("://")
                || value.as_bytes().windows(6).any(|digits| digits.iter().all(u8::is_ascii_digit))
        }) {
        return false;
    }
    // A change to a number without nearby words would be a dangerous global rule.
    if [term, replacement].iter().any(|value| value.chars().any(|c| c.is_ascii_digit())) {
        return [term, replacement].iter().all(|value| {
            value.split_whitespace().count() >= 3
                && value.chars().any(char::is_alphabetic)
        });
    }
    true
}

pub(crate) fn save_suggestion(app:&AppHandle,term:&str,replacement:&str)->Result<bool,String>{
    let mut db=open(app)?;
    save_suggestion_in_connection(&mut db,term,replacement)
}

fn save_suggestion_in_connection(db:&mut Connection,term:&str,replacement:&str)->Result<bool,String>{
    if !reviewable_correction(term,replacement) {return Err("Corrección inválida o sin contexto suficiente".into())}
    let count:i64=db.query_row("SELECT count(*) FROM dictionary",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    let single_words = clear_single_word_correction(term,replacement);
    let protected_rule:bool=db.query_row("SELECT coalesce(m.source,'manual')='manual' OR coalesce(m.enabled,1)=0 FROM dictionary d LEFT JOIN dictionary_meta m ON m.term=d.term WHERE d.term=?1",[term],|r|r.get(0)).optional().map_err(|e|e.to_string())?.unwrap_or(false);
    if single_words && !protected_rule && count<500 {
        let tx=db.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO dictionary(term,replacement) VALUES(?1,?2) ON CONFLICT(term) DO UPDATE SET replacement=excluded.replacement",params![term,replacement]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO dictionary_meta(term,source,enabled) VALUES(?1,'learned',1) ON CONFLICT(term) DO UPDATE SET source='learned',enabled=1",[term]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM suggestions WHERE term=?1",[term]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(true);
    }
    let tx=db.transaction().map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM suggestions WHERE term=?1",[term]).map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO suggestions(term,replacement) VALUES(?1,?2)",params![term,replacement]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM suggestions WHERE rowid NOT IN (SELECT rowid FROM suggestions ORDER BY rowid DESC LIMIT 100)",[]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(false)
}

#[cfg(test)]
mod learning_tests {
    use super::{apply_entries, clear_single_word_correction, is_vocabulary_prompt_term, reviewable_correction, save_suggestion_in_connection, DictionaryEntry};
    use rusqlite::Connection;
    #[test]
    fn learns_only_single_word_replacements_automatically() {
        assert!(clear_single_word_correction("Rook", "RUC"));
        assert!(clear_single_word_correction("postgre", "PostgreSQL"));
        assert!(clear_single_word_correction("girosol", "girasoles"));
        assert!(clear_single_word_correction("giresel", "girasoles"));
        assert!(clear_single_word_correction("Girosol", "Girasoles"));
        assert!(!clear_single_word_correction("10", "10409027276"));
        assert!(!clear_single_word_correction("tí", "que"));
        assert!(!clear_single_word_correction("otra frase", "texto nuevo"));
        assert!(!clear_single_word_correction("clave", "usuario@example.com"));
        assert!(!clear_single_word_correction("hola", "bola"));
        assert!(!clear_single_word_correction("girasol", "girasoles"));
        assert!(!clear_single_word_correction("girasoles", "girasol"));
        assert!(!clear_single_word_correction("amigos", "amigas"));
        assert!(!clear_single_word_correction("profesor", "profesora"));
        assert!(!clear_single_word_correction("camino", "destino"));
    }

    #[test]
    fn numerical_edits_need_context_and_review() {
        assert!(!reviewable_correction("13x", "3x"));
        assert!(reviewable_correction("ecuación es 13x + 4", "ecuación es 3x + 4"));
        assert!(!clear_single_word_correction("ecuación es 13x + 4", "ecuación es 3x + 4"));
        assert!(!reviewable_correction("RUC 10409027276 es", "RUC 10409027270 es"));
        assert!(!reviewable_correction("usuario@example.com", "usuario@example.org"));
    }

    #[test]
    fn math_corrections_do_not_bias_whisper_prompt() {
        assert!(is_vocabulary_prompt_term("RUC"));
        assert!(is_vocabulary_prompt_term("PostgreSQL"));
        assert!(!is_vocabulary_prompt_term("ecuación es 3x + 4"));
        assert!(!is_vocabulary_prompt_term("10409027276"));
    }

    #[test]
    fn learned_word_replaces_future_transcripts_and_can_be_disabled() {
        let mut entries = vec![DictionaryEntry {
            term: "girosol".into(), replacement: "girasoles".into(),
            source: "learned".into(), enabled: true,
        }];
        let corrected = apply_entries(&entries, "Vi girosol. Los girasoles crecieron.").unwrap();
        assert_eq!(corrected.text, "Vi girasoles. Los girasoles crecieron.");
        assert_eq!(corrected.changes, 1);
        entries[0].enabled = false;
        assert_eq!(apply_entries(&entries, "Vi girosol.").unwrap().text, "Vi girosol.");
    }

    #[test]
    fn clear_correction_is_saved_but_manual_and_disabled_rules_are_preserved() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE dictionary(term TEXT PRIMARY KEY COLLATE NOCASE,replacement TEXT NOT NULL);
            CREATE TABLE dictionary_meta(term TEXT PRIMARY KEY COLLATE NOCASE,source TEXT,enabled INTEGER);
            CREATE TABLE suggestions(term TEXT PRIMARY KEY COLLATE NOCASE,replacement TEXT);").unwrap();
        assert!(save_suggestion_in_connection(&mut db,"girosol","girasoles").unwrap());
        let stored:String = db.query_row("SELECT replacement FROM dictionary WHERE term='girosol'",[],|r|r.get(0)).unwrap();
        assert_eq!(stored,"girasoles");
        db.execute("UPDATE dictionary_meta SET enabled=0 WHERE term='girosol'",[]).unwrap();
        assert!(!save_suggestion_in_connection(&mut db,"girosol","girasoles").unwrap());
        let enabled:i64 = db.query_row("SELECT enabled FROM dictionary_meta WHERE term='girosol'",[],|r|r.get(0)).unwrap();
        assert_eq!(enabled,0);
        db.execute("UPDATE dictionary_meta SET source='manual',enabled=1 WHERE term='girosol'",[]).unwrap();
        assert!(!save_suggestion_in_connection(&mut db,"girosol","girasoles").unwrap());
        let source:String = db.query_row("SELECT source FROM dictionary_meta WHERE term='girosol'",[],|r|r.get(0)).unwrap();
        assert_eq!(source,"manual");
    }
}

#[tauri::command]
pub fn dictionary_suggestions(app:AppHandle)->Result<Vec<DictionaryEntry>,String>{
    let db=open(&app)?;
    let mut query=db.prepare("SELECT term,replacement FROM suggestions ORDER BY rowid DESC LIMIT 100").map_err(|e|e.to_string())?;
    let rows=query.query_map([],|r|Ok(DictionaryEntry{term:r.get(0)?,replacement:r.get(1)?,source:"suggestion".into(),enabled:false})).map_err(|e|e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[tauri::command]
pub async fn dictionary_review(app:AppHandle,term:String,replacement:String,accept:bool)->Result<(),String>{
    if accept {
        dictionary_upsert(app.clone(),term.clone(),replacement).await?;
        open(&app)?.execute("INSERT INTO dictionary_meta(term,source,enabled) VALUES(?1,'learned',1) ON CONFLICT(term) DO UPDATE SET source='learned',enabled=1",[&term]).map_err(|e|e.to_string())?;
        crate::notice::show(&app,"Corrección guardada en el diccionario");
    }
    open(&app)?.execute("DELETE FROM suggestions WHERE term=?1",[&term]).map_err(|e|e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn dictionary_set_enabled(app:AppHandle,term:String,enabled:bool)->Result<(),String>{
    open(&app)?.execute("INSERT INTO dictionary_meta(term,enabled) VALUES(?1,?2) ON CONFLICT(term) DO UPDATE SET enabled=excluded.enabled",params![term,enabled]).map_err(|e|e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn dictionary_preview(app: AppHandle, text: String) -> Result<DictionaryPreview, String> {
    if text.chars().count() > 10_000 {
        return Err("La vista previa no puede superar 10 000 caracteres".into());
    }
    tauri::async_runtime::spawn_blocking(move || apply_dictionary(&app, &text))
        .await
        .map_err(|error| format!("La vista previa terminó inesperadamente: {error}"))?
}
