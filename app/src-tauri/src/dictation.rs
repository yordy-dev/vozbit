use crate::audio::{self, CapturedInput};
use crate::delivery::{self, DeliveryStatus, DeliveryTarget};
use crate::storage;
use crate::streaming;
use crate::transcription::{self, TranscriptionEngine, TranscriptionResult};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, LogicalSize, State};

const MAX_DURATION: Duration = Duration::from_secs(120);
static SESSION_CANCELLED:AtomicBool=AtomicBool::new(false);
static OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);
const ERROR_DISPLAY_TIME: Duration = Duration::from_millis(2800);

struct Session {
    stop: Arc<AtomicBool>,
    task: tauri::async_runtime::JoinHandle<Result<CapturedInput, String>>,
    target_tracker: Option<delivery::TargetTracker>,
    auto_insert: bool,
    language: String,
    engine: TranscriptionEngine,
    prefetch: Option<streaming::StreamSession>,
    parakeet_preload: Option<transcription::ParakeetPreload>,
    indicator_visible: bool,
}

#[derive(Default)]
pub struct DictationState(Mutex<Option<Session>>);

#[derive(Default)]
pub struct LastAudioState(Mutex<Option<Vec<f32>>>);

#[derive(Default)]
pub struct LastResultState(Mutex<Option<DictationResult>>);

#[derive(Clone, Serialize)]
pub struct DictationStart {
    max_seconds: u64,
}

#[derive(Clone, Serialize)]
pub struct DictationResult {
    transcript: TranscriptionResult,
    captured_seconds: f64,
    dropped_chunks: u32,
    dictionary_changes: usize,
    dictionary_warning: Option<String>,
    delivery: DeliveryStatus,
    post_release_ms: u128,
    prefetched_segments: u32,
    streaming_used: bool,
}

fn take_session(state: &DictationState) -> Result<Session, String> {
    state
        .0
        .lock()
        .map_err(|_| "No se pudo acceder al estado de grabación".to_string())?
        .take()
        .ok_or_else(|| "No hay una grabación activa".to_string())
}

fn show_overlay(app: &AppHandle, target: Option<DeliveryTarget>) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or("No se encontró el indicador visual de grabación")?;
    let requested_size = LogicalSize::new(216_f64, 58_f64);
    window
        .set_size(requested_size)
        .map_err(|error| format!("No se pudo ajustar el indicador visual: {error}"))?;
    let target_monitor = target.and_then(delivery::window_center).and_then(|(x, y)| {
        window.available_monitors().ok()?.into_iter().find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            x >= position.x && y >= position.y
                && x < position.x + size.width as i32
                && y < position.y + size.height as i32
        })
    });
    if let Some(monitor) = target_monitor.or_else(|| window.current_monitor().ok().flatten()) {
        if let Ok(size) = window.outer_size() {
            let position = monitor.position();
            let area = monitor.size();
            let placement = storage::read_setting(app, "overlay_position")
                .ok()
                .flatten()
                .unwrap_or_else(|| "center".into());
            let x = position.x + (area.width as i32 - size.width as i32) / 2;
            let y = match placement.as_str() {
                "top_center" => position.y + 36,
                "bottom_center" => {
                    position.y + area.height as i32 - size.height as i32 - 58
                }
                _ => position.y + (area.height as i32 - size.height as i32) / 2,
            };
            let _ = window.set_position(PhysicalPosition::new(x, y));
            if storage::read_setting(app, "overlay_custom").ok().flatten().as_deref() == Some("true") {
                if let (Some(x),Some(y)) = (storage::read_setting(app,"overlay_x").ok().flatten().and_then(|s|s.parse::<i32>().ok()), storage::read_setting(app,"overlay_y").ok().flatten().and_then(|s|s.parse::<i32>().ok())) {
                    if let Ok(monitors) = window.available_monitors() {
                        if monitors.iter().any(|m| x >= m.position().x && y >= m.position().y && x + size.width as i32 <= m.position().x + m.size().width as i32 && y + size.height as i32 <= m.position().y + m.size().height as i32) {
                            let _ = window.set_position(PhysicalPosition::new(x,y));
                        }
                    }
                }
            }
        }
    }
    let hold = storage::read_setting(app,"push_to_talk").ok().flatten().as_deref() == Some("true");
    let _ = app.emit("overlay://config", OverlayConfig {hold});
    window
        .show()
        .map_err(|error| format!("No se pudo mostrar el indicador visual de grabación: {error}"))?;
    OVERLAY_GENERATION.fetch_add(1, Ordering::AcqRel);
    let _ = app.emit("dictation://state", "recording");
    Ok(())
}

#[derive(Clone, Serialize)]
struct OverlayConfig { hold: bool }

#[tauri::command]
pub fn overlay_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "overlay" { return Err("Ventana no válida".into()); }
    window.start_dragging().map_err(|e|e.to_string())
}

#[tauri::command]
pub fn dictation_status(last_audio: State<'_, LastAudioState>) -> bool {
    last_audio.0.lock().map(|a|a.is_some()).unwrap_or(false)
}

#[tauri::command]
pub fn dictation_last_result(last_result: State<'_, LastResultState>) -> Option<DictationResult> {
    last_result.0.lock().ok()?.clone()
}

#[tauri::command]
pub fn transcription_cancel() { SESSION_CANCELLED.store(true,Ordering::Release); transcription::cancel(); }

pub(crate) fn hide_overlay(app: &AppHandle) {
    OVERLAY_GENERATION.fetch_add(1, Ordering::AcqRel);
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

fn show_overlay_error(app: &AppHandle, message: &'static str) {
    let _ = app.emit("overlay://message", message);
    let _ = app.emit("dictation://state", "error");
    let generation = OVERLAY_GENERATION.load(Ordering::Acquire);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(ERROR_DISPLAY_TIME);
        if OVERLAY_GENERATION.load(Ordering::Acquire) == generation {
            hide_overlay(&app);
            let _ = app.emit("dictation://state", "idle");
        }
    });
}

#[tauri::command]
pub async fn dictation_start(
    app: AppHandle,
    state: State<'_, DictationState>,
    device_id: Option<String>,
    delivery_requested: bool,
    language: String,
) -> Result<DictationStart, String> {
    if !matches!(language.as_str(), "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    crate::learning::stop();
    SESSION_CANCELLED.store(false,Ordering::Release);
    let engine = transcription::selected_engine(&app)?;
    if engine == TranscriptionEngine::ParakeetUltra {
        transcription::verify_parakeet_resources(&app)?;
    }
    let auto_insert = delivery_requested
        && storage::read_setting(&app, "auto_insert")?.as_deref() != Some("false");
    let ready_rx = {
        let mut session_slot = state
            .0
            .lock()
            .map_err(|_| "No se pudo acceder al estado de grabación".to_string())?;
        if session_slot.is_some() {
            return Err("Ya hay una grabación en curso".into());
        }
        let target_tracker = auto_insert.then(delivery::TargetTracker::start);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_worker = stop.clone();
        let worker_app = app.clone();
        let prefetch = if engine == TranscriptionEngine::Whisper {
            streaming::StreamSession::start(app.clone(), language.clone()).ok()
        } else { None };
        let parakeet_preload = if engine == TranscriptionEngine::ParakeetUltra {
            transcription::ParakeetPreload::start(&app).ok()
        } else { None };
        let prefetch_sink = prefetch.as_ref().map(streaming::StreamSession::sink);
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let ready_error = ready_tx.clone();
        let task = tauri::async_runtime::spawn_blocking(move || {
            let result = audio::capture_input(
                &worker_app,
                device_id,
                MAX_DURATION,
                stop_for_worker,
                Some(ready_tx),
                prefetch_sink,
            );
            if let Err(error) = &result {
                let _ = ready_error.try_send(Err(error.clone()));
                let _ = worker_app.emit("dictation://error", error.clone());
            }
            result
        });
        *session_slot = Some(Session {
            stop,
            task,
            target_tracker,
            auto_insert,
            language,
            engine,
            prefetch,
            parakeet_preload,
            indicator_visible: delivery_requested,
        });
        ready_rx
    };

    let startup =
        tauri::async_runtime::spawn_blocking(move || ready_rx.recv_timeout(Duration::from_secs(5)))
            .await
            .map_err(|error| format!("La preparación del micrófono falló: {error}"))?;
    match startup {
        Ok(Ok(())) => {
            if delivery_requested {
                let overlay_target = state.0.lock().ok().and_then(|slot|
                    slot.as_ref().and_then(|session| session.target_tracker.as_ref().and_then(delivery::TargetTracker::latest)));
                if let Err(error) = show_overlay(&app, overlay_target) {
                    if let Ok(session) = take_session(&state) {
                        session.stop.store(true, Ordering::Release);
                        let _ = session.task.await;
                        if let Some(prefetch) = session.prefetch {
                            let _ = tauri::async_runtime::spawn_blocking(move || prefetch.cancel()).await;
                        }
                    }
                    return Err(error);
                }
            }
            Ok(DictationStart {
                max_seconds: MAX_DURATION.as_secs(),
            })
        }
        outcome => {
            if let Ok(session) = take_session(&state) {
                session.stop.store(true, Ordering::Release);
                let _ = session.task.await;
                if let Some(prefetch) = session.prefetch {
                    let _ = tauri::async_runtime::spawn_blocking(move || prefetch.cancel()).await;
                }
            }
            Err(match outcome {
                Ok(Err(error)) => error,
                Err(_) => "El micrófono no respondió a tiempo".into(),
                Ok(Ok(())) => unreachable!(),
            })
        }
    }
}

#[tauri::command]
pub async fn dictation_stop(
    app: AppHandle,
    state: State<'_, DictationState>,
    last_audio: State<'_, LastAudioState>,
    last_result: State<'_, LastResultState>,
    language: String,
) -> Result<DictationResult, String> {
    if !matches!(language.as_str(), "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    let session = take_session(&state)?;
    let indicator_visible = session.indicator_visible;
    let target_tracker = session.target_tracker;
    let auto_insert = session.auto_insert;
    let session_language = session.language;
    let engine = session.engine;
    let prefetched_segments = session.prefetch.as_ref().map(streaming::StreamSession::completed_before_stop).unwrap_or(0);
    let prefetch = session.prefetch;
    let parakeet_preload = session.parakeet_preload;
    let released_at = Instant::now();
    let _ = app.emit("dictation://state", "processing");
    let result: Result<DictationResult,String> = async {
        session.stop.store(true, Ordering::Release);
        let captured = session
            .task
            .await
            .map_err(|error| format!("La grabación terminó inesperadamente: {error}"))??;
        let captured_seconds = captured.samples.len() as f64 / 16_000.0;
        let dropped_chunks = captured.result.dropped_chunks;
        if let Ok(mut stored) = last_audio.0.lock() {
            *stored = Some(captured.samples.clone());
        }
        let worker_app = app.clone();
        let (transcript, dictionary_changes, dictionary_warning, delivery, streaming_used) =
            tauri::async_runtime::spawn_blocking(move || {
                let (mut transcript, streaming_used) = match prefetch {
                    Some(prefetch) if session_language == language => {
                        match prefetch.finish(captured.result.input_frames) {
                            Ok(streamed) => match transcription::result_from_raw(
                                &worker_app, &streamed.raw_text, &language, captured_seconds, streamed.engine_ms,
                            ) {
                                Ok(result) => (result, true),
                                Err(_) => (transcription::transcribe_pcm_with_engine(&worker_app, &captured.samples, &language, engine)?, false),
                            },
                            Err(_) => (transcription::transcribe_pcm_with_engine(&worker_app, &captured.samples, &language, engine)?, false),
                        }
                    }
                    Some(prefetch) => {
                        prefetch.cancel();
                        (transcription::transcribe_pcm_with_engine(&worker_app, &captured.samples, &language, engine)?, false)
                    }
                    None if engine == TranscriptionEngine::ParakeetUltra => (
                        transcription::transcribe_pcm_with_preload(&worker_app, &captured.samples, &language, parakeet_preload)?, false
                    ),
                    None => (transcription::transcribe_pcm_with_engine(&worker_app, &captured.samples, &language, engine)?, false),
                };
                if SESSION_CANCELLED.load(Ordering::Acquire){return Err("Dictado cancelado".into());}
                let mut changes = 0;
                let warning = match storage::apply_dictionary(&worker_app, &transcript.text) {
                    Ok(preview) => {
                        transcript.text = preview.text;
                        changes = preview.changes;
                        None
                    }
                    Err(error) => Some(format!(
                        "El texto se obtuvo, pero no se pudo aplicar el diccionario: {error}"
                    )),
                };
                if SESSION_CANCELLED.load(Ordering::Acquire) { return Err("Dictado cancelado".into()); }
                if indicator_visible { let _ = worker_app.emit("dictation://state", "completing"); }
                if SESSION_CANCELLED.load(Ordering::Acquire) { return Err("Dictado cancelado".into()); }
                let delivery_target = target_tracker.as_ref().and_then(delivery::TargetTracker::snapshot);
                let delivery = if auto_insert {
                    let mut delivered_text = transcript.text.clone();
                    if storage::read_setting(&worker_app, "append_trailing_space")?
                        .as_deref()
                        != Some("false")
                    {
                        delivered_text.push(' ');
                    }
                    let status = delivery::deliver(target_tracker.as_ref(), delivery_target, &delivered_text);
                    if status.sent { if let Some(pid)=delivery::process_id(delivery_target) { crate::learning::observe(&worker_app,pid,&delivered_text); } }
                    status
                } else {
                    delivery::skipped()
                };
                if indicator_visible && (!auto_insert || delivery.sent) {
                    let _ = worker_app.emit("dictation://state", "done");
                    std::thread::sleep(Duration::from_millis(220));
                }
                Ok::<_, String>((transcript, changes, warning, delivery, streaming_used))
            })
            .await
            .map_err(|error| format!("La inferencia terminó inesperadamente: {error}"))??;
        Ok(DictationResult {
            transcript,
            captured_seconds,
            dropped_chunks,
            dictionary_changes,
            dictionary_warning,
            delivery,
            post_release_ms: released_at.elapsed().as_millis(),
            prefetched_segments,
            streaming_used,
        })
    }
    .await;
    if let Ok(completed) = &result {
        if let Ok(mut slot) = last_result.0.lock() { *slot = Some(completed.clone()); }
    }
    match &result {
        Ok(r) if auto_insert && !r.delivery.sent => {
            if indicator_visible { show_overlay_error(&app, "No se pudo insertar"); }
            else { hide_overlay(&app); let _ = app.emit("dictation://state", "idle"); }
            crate::notice::show(&app,r.delivery.reason.as_deref().unwrap_or("No se pudo entregar. Copia el texto desde Inicio."));
        },
        Ok(_) => { hide_overlay(&app); let _ = app.emit("dictation://state", "idle"); },
        Err(e) if e == "Dictado cancelado" || e == "Transcripción cancelada" => {
            hide_overlay(&app);
            let _ = app.emit("dictation://state", "idle");
        },
        Err(e) => {
            if indicator_visible { show_overlay_error(&app, "Falló la transcripción"); }
            else { hide_overlay(&app); let _ = app.emit("dictation://state", "idle"); }
            crate::notice::show(&app,e);
        },
    }
    result
}

#[tauri::command]
pub async fn dictation_reprocess(
    app: AppHandle,
    last_audio: State<'_, LastAudioState>,
    last_result: State<'_, LastResultState>,
    language: String,
) -> Result<DictationResult, String> {
    SESSION_CANCELLED.store(false,Ordering::Release);
    if !matches!(language.as_str(), "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    let samples = last_audio
        .0
        .lock()
        .map_err(|_| "No se pudo acceder a la última grabación".to_string())?
        .clone()
        .ok_or_else(|| {crate::notice::show(&app,"Todavía no hay una grabación para reprocesar"); "Todavía no hay una grabación para reprocesar".to_string()})?;
    let _ = app.emit("dictation://state", "processing");
    let released_at = Instant::now();
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let captured_seconds = samples.len() as f64 / 16_000.0;
        let mut transcript = transcription::transcribe_pcm(&worker_app, &samples, &language)?;
        let mut dictionary_changes = 0;
        let dictionary_warning = match storage::apply_dictionary(&worker_app, &transcript.text) {
            Ok(preview) => {
                transcript.text = preview.text;
                dictionary_changes = preview.changes;
                None
            }
            Err(error) => Some(format!("El texto se obtuvo, pero no se pudo aplicar el diccionario: {error}")),
        };
        Ok::<_, String>(DictationResult {
            transcript,
            captured_seconds,
            dropped_chunks: 0,
            dictionary_changes,
            dictionary_warning,
            delivery: delivery::skipped(),
            post_release_ms: released_at.elapsed().as_millis(),
            prefetched_segments: 0,
            streaming_used: false,
        })
    })
    .await
    .map_err(|error| format!("El reprocesamiento terminó inesperadamente: {error}")).and_then(|result|result);
    let _ = app.emit("dictation://state", "idle");
    if let Ok(completed) = &result {
        if let Ok(mut slot) = last_result.0.lock() { *slot = Some(completed.clone()); }
    }
    if let Err(error)=&result{crate::notice::show(&app,error);}
    result
}

#[tauri::command]
pub async fn dictation_cancel(
    app: AppHandle,
    state: State<'_, DictationState>,
    show_error: Option<bool>,
) -> Result<(), String> {
    let session = take_session(&state)?;
    let indicator_visible = session.indicator_visible;
    session.stop.store(true, Ordering::Release);
    let _ = session.task.await;
    if let Some(prefetch) = session.prefetch {
        let _ = tauri::async_runtime::spawn_blocking(move || prefetch.cancel()).await;
    }
    if show_error.unwrap_or(false) {
        if indicator_visible { show_overlay_error(&app, "Error de grabación"); }
        else { hide_overlay(&app); let _ = app.emit("dictation://state", "idle"); }
        crate::notice::show(&app,"No se pudo grabar. Revisa el micrófono y vuelve a intentar.");
    } else {
        hide_overlay(&app);
        let _ = app.emit("dictation://state", "idle");
    }
    Ok(())
}
