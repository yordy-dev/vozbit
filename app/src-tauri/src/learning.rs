use crate::storage;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
const OBSERVE_SHA256: &str = "d368d6efb81fe54d9a64d4d0a4fd1ddc43f5a660bcdbc715bbee320af800c408";
const DIFF_SHA256: &str = "8b00158683e9749e51619e263bed62f4aa4d954bebd6701b80f64921c17f5cd4";
const OBSERVATION_LIMIT: Duration = Duration::from_secs(30);
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}
pub fn observe(app: &AppHandle, pid: u32, text: &str) {
    launch(app, pid, text, false);
}
pub fn selected_word(app: &AppHandle) {
    if let Some(pid) = crate::delivery::process_id(crate::delivery::capture_target()) {
        launch(app, pid, "", true);
    }
}
fn launch(app: &AppHandle, pid: u32, text: &str, selection: bool) {
    stop();
    if storage::read_setting(app, "private_mode")
        .ok()
        .flatten()
        .as_deref()
        == Some("true")
    {
        return;
    }
    if !selection
        && (storage::read_setting(app, "learn_corrections")
            .ok()
            .flatten()
            .as_deref()
            != Some("true")
            || storage::read_setting(app, "learning_consent_v048")
                .ok()
                .flatten()
                .as_deref()
                != Some("true"))
    {
        return;
    }
    let generation = GENERATION.load(Ordering::Relaxed);
    let app = app.clone();
    let text = text.to_string();
    std::thread::spawn(move || {
        let Ok(path) = app.path().resolve(
            "resources/observe-correction.ps1",
            tauri::path::BaseDirectory::Resource,
        ) else {
            return;
        };
        let diff = path.with_file_name("correction-diff.ps1");
        let resources_ok = crate::transcription::sha256(&path).ok().as_deref()
            == Some(OBSERVE_SHA256)
            && crate::transcription::sha256(&diff).ok().as_deref() == Some(DIFF_SHA256);
        if !resources_ok {
            crate::notice::show(&app,"El aprendizaje se desactivó porque sus recursos no pasaron la comprobación de integridad.");
            return;
        }
        let Some(system_root) = std::env::var_os("SystemRoot") else {
            return;
        };
        let powershell =
            PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        if !powershell.is_file() {
            return;
        }
        let Ok(mut child) = Command::new(powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(path)
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        if let Some(mut input) = child.stdin.take() {
            let _ = writeln!(
                input,
                "{}",
                serde_json::json!({"pid":pid,"owner":std::process::id(),"text":text,"selection":selection})
            );
        }
        let Some(output) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let started = Instant::now();
        let mut observed = 0_usize;
        loop {
            while let Ok(line) = receiver.try_recv() {
                if observed < 20 {
                    observed += apply_observation(&app, &line, selection);
                }
            }
            if GENERATION.load(Ordering::Relaxed) != generation
                || started.elapsed() > OBSERVATION_LIMIT
            {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return;
            }
            match child.try_wait() {
                Ok(Some(_)) => break,
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return;
                }
                _ => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let _ = child.wait();
        let _ = reader.join();
        while let Ok(line) = receiver.try_recv() {
            if observed < 20 {
                observed += apply_observation(&app, &line, selection);
            }
        }
        if selection && observed == 0 {
            crate::notice::show(
                &app,
                "No se pudo leer la selección. Agrégala desde Diccionario.",
            );
        }
    });
}
#[derive(Clone, serde::Deserialize)]
struct Candidate {
    term: String,
    replacement: String,
    context_term: Option<String>,
    context_replacement: Option<String>,
}

#[derive(serde::Deserialize)]
struct ObservationBatch {
    #[serde(rename = "original")]
    _original: String,
    #[serde(rename = "edited")]
    _edited: String,
    candidates: Vec<Candidate>,
}

fn apply_observation(app: &AppHandle, line: &str, selection: bool) -> usize {
    if selection {
        let Ok(candidate) = serde_json::from_str::<Candidate>(line) else {
            return 0;
        };
        let result = tauri::async_runtime::block_on(storage::dictionary_upsert(
            app.clone(),
            candidate.term,
            candidate.replacement,
        ));
        if result.is_ok() {
            let _ = app.emit("learning://updated", ());
            crate::notice::show(app, "Palabra seleccionada agregada al diccionario");
        } else {
            crate::notice::show(
                app,
                "Selecciona una palabra o expresión de hasta 80 caracteres",
            );
        }
        return usize::from(result.is_ok());
    }

    let Ok(batch) = serde_json::from_str::<ObservationBatch>(line) else {
        return 0;
    };
    let mut learned = Vec::new();
    let mut reviewed = 0;
    for candidate in batch.candidates.iter().take(20) {
        if locally_learnable(candidate) {
            if storage::save_auto_learned_word(app, &candidate.term, &candidate.replacement).is_ok()
            {
                learned.push(candidate.replacement.clone());
            }
        } else {
            let _ =
                storage::queue_learning_suggestion(app, &candidate.term, &candidate.replacement);
        }
        reviewed += 1;
    }
    if reviewed > 0 {
        let _ = app.emit("learning://updated", ());
    }
    if !learned.is_empty() {
        let message = if learned.len() == 1 {
            format!(
                "Palabra aprendida: «{}». Puedes deshacerla en Diccionario.",
                learned[0]
            )
        } else {
            let preview = learned
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Palabras aprendidas: {preview}{}. Revísalas en Diccionario.",
                if learned.len() > 3 { "…" } else { "" }
            )
        };
        crate::notice::show(app, &message);
    }
    reviewed
}

fn locally_learnable(candidate: &Candidate) -> bool {
    let valid = |value: &str| {
        (2..=64).contains(&value.chars().count())
            && !value.chars().any(char::is_whitespace)
            && value.chars().all(|character| {
                character.is_alphanumeric()
                    || matches!(character, '-' | '_' | '.' | '+' | '#' | '\'' | '’')
            })
            && value.chars().any(char::is_alphabetic)
            && !value.contains("..")
    };
    let _ = (&candidate.context_term, &candidate.context_replacement);
    valid(candidate.term.trim())
        && valid(candidate.replacement.trim())
        && candidate.term.trim() != candidate.replacement.trim()
}

#[cfg(test)]
mod tests {
    use super::{locally_learnable, Candidate};

    fn candidate(term: &str, replacement: &str) -> Candidate {
        Candidate {
            term: term.into(),
            replacement: replacement.into(),
            context_term: None,
            context_replacement: None,
        }
    }

    #[test]
    fn only_safe_single_words_are_learned_locally() {
        assert!(locally_learnable(&candidate("Rook", "RUC")));
        assert!(locally_learnable(&candidate("postgre", "PostgreSQL")));
        assert!(locally_learnable(&candidate("postgresql", "PostgreSQL")));
        assert!(locally_learnable(&candidate("giresel", "girasoles")));
        assert!(!locally_learnable(&candidate("hola amigo", "hola, amigo")));
        assert!(!locally_learnable(&candidate("https://uno", "https://dos")));
        assert!(!locally_learnable(&candidate("13", "3")));
    }
}
