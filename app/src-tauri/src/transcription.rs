use crate::{audio, formatting, storage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const MODEL_SHA256: &str = "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898";
const VAD_SHA256: &str = "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987";
const PARAKEET_MODEL_SHA256: &str = "c2fb452a9df468a141012b01c8c168a25ce93f710897c7de6e353c6cc250986a";
const PARAKEET_CLI_SHA256: &str = "d5c164ce0f5e83e674260d9fb5c67025c9c38a935c9ccd1f68793fc1c7cc50b7";
const PARAKEET_VAD_SHA256: &str = "8160489282352accc0e95925c2f6bf3d76fb8f7bccdce5c7e46808cb15e8443c";
const _: () = {
    assert!(MODEL_SHA256.len() == 64);
    assert!(VAD_SHA256.len() == 64);
    assert!(PARAKEET_MODEL_SHA256.len() == 64);
    assert!(PARAKEET_CLI_SHA256.len() == 64);
    assert!(PARAKEET_VAD_SHA256.len() == 64);
};
static TRANSCRIPTION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
pub fn cancel() { CANCEL_REQUESTED.store(true, Ordering::Release); }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TranscriptionEngine { Whisper, ParakeetUltra }

impl TranscriptionEngine {
    fn name(self) -> &'static str {
        match self {
            Self::Whisper => "Whisper base q5_1 multilingüe",
            Self::ParakeetUltra => "Parakeet Ultra Q8_0",
        }
    }
}

pub(crate) fn selected_engine(app: &AppHandle) -> Result<TranscriptionEngine, String> {
    match storage::read_setting(app, "transcription_engine")?.as_deref() {
        None | Some("whisper") => Ok(TranscriptionEngine::Whisper),
        Some("parakeet_ultra") => Ok(TranscriptionEngine::ParakeetUltra),
        Some(_) => Err("El motor guardado no es válido".into()),
    }
}

struct TranscriptionGuard;

struct TemporaryAudio(PathBuf);

impl Drop for TemporaryAudio {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl TranscriptionGuard {
    fn acquire() -> Result<Self, String> {
        TRANSCRIPTION_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Ya se está transcribiendo un audio".to_string())?;
        CANCEL_REQUESTED.store(false, Ordering::Release);
        Ok(Self)
    }
}

impl Drop for TranscriptionGuard {
    fn drop(&mut self) {
        TRANSCRIPTION_ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
pub struct ModelStatus {
    name: &'static str,
    available: bool,
    sha256_ok: bool,
    engine_available: bool,
    size_bytes: u64,
    vad_available: bool,
    vad_sha256_ok: bool,
    parakeet_available: bool,
    parakeet_sha256_ok: bool,
    parakeet_engine_available: bool,
    parakeet_vad_sha256_ok: bool,
    parakeet_size_bytes: u64,
}

#[derive(Clone, Serialize)]
pub struct TranscriptionResult {
    pub(crate) text: String,
    requested_language: String,
    audio_seconds: f64,
    inference_ms: u128,
    model_name: &'static str,
}

fn resources(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    use tauri::path::BaseDirectory;
    let model = app
        .path()
        .resolve(
            "resources/models/ggml-base-q5_1.bin",
            BaseDirectory::Resource,
        )
        .map_err(|error| format!("No se encontró la ruta del modelo: {error}"))?;
    let engine = app
        .path()
        .resolve("resources/whisper/whisper-cli.exe", BaseDirectory::Resource)
        .map_err(|error| format!("No se encontró la ruta del motor: {error}"))?;
    Ok((model, engine))
}

fn parakeet_resources(app: &AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    use tauri::path::BaseDirectory;
    let model = app.path().resolve("resources/models/ultra-q8_0.gguf", BaseDirectory::Resource)
        .map_err(|error| format!("No se encontró la ruta de Parakeet Ultra: {error}"))?;
    let engine = app.path().resolve("resources/parakeet/parakeet-cli.exe", BaseDirectory::Resource)
        .map_err(|error| format!("No se encontró el motor Parakeet: {error}"))?;
    let vad = app.path().resolve("resources/models/silero-vad-f16.gguf", BaseDirectory::Resource)
        .map_err(|error| format!("No se encontró el detector de voz de Parakeet: {error}"))?;
    Ok((model, engine, vad))
}

pub(crate) fn verify_parakeet_resources(app: &AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let (model, engine, vad) = parakeet_resources(app)?;
    for (path, expected, label) in [
        (&model, PARAKEET_MODEL_SHA256, "modelo Parakeet Ultra"),
        (&engine, PARAKEET_CLI_SHA256, "motor Parakeet"),
        (&vad, PARAKEET_VAD_SHA256, "detector de voz Parakeet"),
    ] {
        if !path.is_file() {
            return Err(format!("No se encontró el {label} en {}", path.display()));
        }
        let actual = sha256(path)?;
        if actual != expected {
            return Err(format!("El {label} no pasó la comprobación de integridad (archivo: {}; SHA-256: {actual})", path.display()));
        }
    }
    Ok((model, engine, vad))
}

fn vad_resource(app: &AppHandle) -> Result<PathBuf, String> {
    use tauri::path::BaseDirectory;
    app.path()
        .resolve(
            "resources/models/ggml-silero-v6.2.0.bin",
            BaseDirectory::Resource,
        )
        .map_err(|error| format!("No se encontró la ruta del detector de voz: {error}"))
}

pub(crate) fn sha256(path: &Path) -> Result<String, String> {
    type Cache = std::collections::HashMap<PathBuf,(u64,SystemTime,String)>;
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Cache>> = std::sync::OnceLock::new();
    let metadata=fs::metadata(path).map_err(|e|e.to_string())?;
    let modified=metadata.modified().map_err(|e|e.to_string())?;
    if let Ok(cache)=CACHE.get_or_init(Default::default).lock() {
        if let Some((size,time,digest))=cache.get(path) { if *size==metadata.len() && *time==modified {return Ok(digest.clone())} }
    }
    let mut file = File::open(path).map_err(|_| "No se pudo abrir el modelo local".to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "No se pudo comprobar la integridad del modelo".to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let digest=format!("{:x}", hasher.finalize());
    if let Ok(mut cache)=CACHE.get_or_init(Default::default).lock(){cache.insert(path.to_path_buf(),(metadata.len(),modified,digest.clone()));}
    Ok(digest)
}

pub(crate) fn verified_model_vad(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let (model, _) = resources(app)?;
    if !model.is_file() || sha256(&model)? != MODEL_SHA256 {
        return Err("El modelo local no pasó la comprobación de integridad".into());
    }
    let vad = vad_resource(app)?;
    if !vad.is_file() || sha256(&vad)? != VAD_SHA256 {
        return Err("El detector local de voz no pasó la comprobación de integridad".into());
    }
    Ok((model, vad))
}

pub(crate) fn initial_prompt(app: &AppHandle, language: &str) -> String {
    let base = match language {
        "es" => "Transcripción en español. Vocabulario: RUC, ecuación, logaritmo, base, exponente, variable x.",
        "en" => "Clear English transcription with natural punctuation, digits and precise mathematical expressions.",
        _ => "",
    };
    let terms = storage::dictionary_prompt_terms(app).unwrap_or_default();
    if terms.is_empty() { base.to_string() }
    else { format!("{base} Vocabulario preferido: {}.", terms.join(", ")) }
}

pub(crate) fn result_from_raw(
    app: &AppHandle,
    raw_text: &str,
    language: &str,
    audio_seconds: f64,
    inference_ms: u128,
) -> Result<TranscriptionResult, String> {
    result_from_raw_for_engine(app, raw_text, language, audio_seconds, inference_ms, TranscriptionEngine::Whisper)
}

pub(crate) fn result_from_raw_for_engine(
    app: &AppHandle,
    raw_text: &str,
    language: &str,
    audio_seconds: f64,
    inference_ms: u128,
    engine: TranscriptionEngine,
) -> Result<TranscriptionResult, String> {
    if raw_text.trim().is_empty() {
        return Err("No se detectó voz en esta grabación".into());
    }
    let text_correction = storage::read_setting(app, "text_correction")?.as_deref() != Some("false");
    let format_numbers = storage::read_setting(app, "format_numbers")?.as_deref() != Some("false");
    let math_symbols = storage::read_setting(app, "math_symbols")?.as_deref() != Some("false");
    let text = formatting::format_transcript(raw_text, language, text_correction, format_numbers, math_symbols);
    Ok(TranscriptionResult {
        text,
        requested_language: language.to_string(),
        audio_seconds,
        inference_ms,
        model_name: engine.name(),
    })
}

#[tauri::command]
pub async fn model_status(app: AppHandle) -> Result<ModelStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (model, engine) = resources(&app)?;
        let size_bytes = model.metadata().map(|item| item.len()).unwrap_or(0);
        let available = size_bytes > 0;
        let sha256_ok = available && sha256(&model)? == MODEL_SHA256;
        let vad = vad_resource(&app)?;
        let vad_available = vad.is_file();
        let vad_sha256_ok = vad_available && sha256(&vad)? == VAD_SHA256;
        let (parakeet_model, parakeet_engine, parakeet_vad) = parakeet_resources(&app)?;
        let parakeet_size_bytes = parakeet_model.metadata().map(|item| item.len()).unwrap_or(0);
        let parakeet_available = parakeet_size_bytes > 0;
        let parakeet_sha256_ok = parakeet_available && sha256(&parakeet_model)? == PARAKEET_MODEL_SHA256;
        let parakeet_engine_available = parakeet_engine.is_file() && sha256(&parakeet_engine)? == PARAKEET_CLI_SHA256;
        let parakeet_vad_sha256_ok = parakeet_vad.is_file() && sha256(&parakeet_vad)? == PARAKEET_VAD_SHA256;
        Ok(ModelStatus {
            name: "Whisper base q5_1 multilingüe",
            available,
            sha256_ok,
            engine_available: engine.is_file(),
            size_bytes,
            vad_available,
            vad_sha256_ok,
            parakeet_available,
            parakeet_sha256_ok,
            parakeet_engine_available,
            parakeet_vad_sha256_ok,
            parakeet_size_bytes,
        })
    })
    .await
    .map_err(|error| format!("La verificación del modelo terminó inesperadamente: {error}"))?
}

fn inspect_wav(path: &Path) -> Result<f64, String> {
    let metadata = path
        .metadata()
        .map_err(|_| "No se pudo leer el WAV de diagnóstico".to_string())?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err("El WAV de diagnóstico supera el límite de 20 MiB".into());
    }
    let mut reader = hound::WavReader::open(path)
        .map_err(|_| "El WAV de diagnóstico no se puede abrir".to_string())?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("Se requiere un WAV PCM mono de 16 kHz y 16 bits".into());
    }
    let mut frames = 0_u64;
    let mut power = 0_f64;
    for sample in reader.samples::<i16>() {
        let value = sample.map_err(|_| "El WAV contiene muestras corruptas".to_string())?;
        frames += 1;
        power += (value as f64 / i16::MAX as f64).powi(2);
    }
    if frames < 16_000 {
        return Err("El audio debe durar al menos un segundo".into());
    }
    if (power / frames as f64).sqrt() < 0.005 {
        return Err("La señal es demasiado baja para transcribir. Haz otra prueba hablando cerca del micrófono".into());
    }
    Ok(frames as f64 / 16_000_f64)
}

#[tauri::command]
pub async fn transcription_transcribe_diagnostic(
    app: AppHandle,
    path: String,
    language: String,
) -> Result<TranscriptionResult, String> {
    let guard = TranscriptionGuard::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        run_transcription(&app, &path, &language)
    })
    .await
    .map_err(|error| format!("La transcripción terminó inesperadamente: {error}"))?
}

fn run_transcription(
    app: &AppHandle,
    path: &str,
    language: &str,
) -> Result<TranscriptionResult, String> {
    if !matches!(language, "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    let allowed = audio::diagnostic_dir(app)?
        .canonicalize()
        .map_err(|_| "No existe la carpeta de diagnósticos".to_string())?;
    let input = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "No existe el WAV seleccionado".to_string())?;
    if !input.starts_with(&allowed)
        || input.extension().and_then(|part| part.to_str()) != Some("wav")
        || !input
            .file_name()
            .and_then(|part| part.to_str())
            .is_some_and(|name| name.starts_with("mic-test-"))
    {
        return Err(
            "Solo se pueden transcribir WAV creados por la prueba de esta aplicación".into(),
        );
    }
    let audio_seconds = inspect_wav(&input)?;
    run_engine(app, &input, language, audio_seconds, selected_engine(app)?)
}

pub(crate) fn transcribe_pcm(
    app: &AppHandle,
    samples: &[f32],
    language: &str,
) -> Result<TranscriptionResult, String> {
    transcribe_pcm_with_engine(app, samples, language, selected_engine(app)?)
}

pub(crate) fn transcribe_pcm_with_engine(
    app: &AppHandle,
    samples: &[f32],
    language: &str,
    engine: TranscriptionEngine,
) -> Result<TranscriptionResult, String> {
    transcribe_pcm_inner(app, samples, language, engine, None)
}

pub(crate) fn transcribe_pcm_with_preload(
    app: &AppHandle,
    samples: &[f32],
    language: &str,
    preload: Option<ParakeetPreload>,
) -> Result<TranscriptionResult, String> {
    transcribe_pcm_inner(app, samples, language, TranscriptionEngine::ParakeetUltra, preload)
}

fn transcribe_pcm_inner(
    app: &AppHandle,
    samples: &[f32],
    language: &str,
    engine: TranscriptionEngine,
    preload: Option<ParakeetPreload>,
) -> Result<TranscriptionResult, String> {
    let _guard = TranscriptionGuard::acquire()?;
    if samples.len() < 16_000 {
        return Err("Graba al menos un segundo antes de transcribir".into());
    }
    let rms = (samples
        .iter()
        .map(|sample| (*sample as f64).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    if rms < 0.005 {
        return Err("La señal es demasiado baja para transcribir. Revisa el micrófono".into());
    }
    let directory =
        if cfg!(debug_assertions) && std::env::var_os("VOICETEXT_DIAGNOSTICS_DIR").is_some() {
            audio::diagnostic_dir(app)?
        } else {
            app.path()
                .app_cache_dir()
                .map_err(|error| {
                    format!("No se encontró la carpeta temporal de la aplicación: {error}")
                })?
                .join("transcription")
        };
    fs::create_dir_all(&directory)
        .map_err(|_| "No se pudo crear la carpeta temporal de audio".to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let input = directory.join(format!("transcribe-{}-{timestamp}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&input, spec)
        .map_err(|_| "No se pudo crear el WAV temporal".to_string())?;
    let temporary = TemporaryAudio(input.clone());
    for sample in samples {
        writer
            .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
            .map_err(|_| "No se pudo escribir el WAV temporal".to_string())?;
    }
    writer
        .finalize()
        .map_err(|_| "No se pudo cerrar el WAV temporal".to_string())?;
    let audio_seconds = samples.len() as f64 / 16_000.0;
    let result = match preload {
        Some(preload) => match preload.finish(app, &input, language, audio_seconds) {
            Ok(result) => Ok(result),
            Err(error) if !CANCEL_REQUESTED.load(Ordering::Acquire) => {
                eprintln!("Parakeet anticipado no disponible ({error}); se usa el motor normal");
                run_engine(app, &input, language, audio_seconds, engine)
            }
            Err(error) => Err(error),
        },
        None => run_engine(app, &input, language, audio_seconds, engine),
    };
    fs::remove_file(&input).map_err(|_| {
        "No se pudo eliminar el WAV temporal; revisa la carpeta de caché".to_string()
    })?;
    drop(temporary);
    result
}

fn run_engine(
    app: &AppHandle,
    input: &Path,
    language: &str,
    audio_seconds: f64,
    engine: TranscriptionEngine,
) -> Result<TranscriptionResult, String> {
    match engine {
        TranscriptionEngine::Whisper => run_whisper(app, input, language, audio_seconds),
        TranscriptionEngine::ParakeetUltra => run_parakeet(app, input, language, audio_seconds),
    }
}

#[derive(Deserialize)]
struct ParakeetOutput { text: String }

pub(crate) struct ParakeetPreload {
    child: Child,
    output_path: PathBuf,
}

impl ParakeetPreload {
    pub(crate) fn start(app: &AppHandle) -> Result<Self, String> {
        let (model, executable, vad) = verify_parakeet_resources(app)?;
        let directory = app.path().app_cache_dir()
            .map_err(|error| format!("No se encontró la carpeta temporal: {error}"))?
            .join("transcription");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let output_path = directory.join(format!("parakeet-preload-{}-{}.tmp",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos()));
        let output_file = File::create(&output_path).map_err(|error| error.to_string())?;
        let threads = std::thread::available_parallelism()
            .map(|count| (count.get() / 2).clamp(1, 4))
            .unwrap_or(2)
            .to_string();
        let mut command = Command::new(&executable);
        command.current_dir(executable.parent().ok_or("Ruta del motor Parakeet inválida")?)
            .args(["transcribe", "--model"])
            .arg(&model)
            .args(["--input", "-", "--json", "--vad", "--vad-model"])
            .arg(&vad)
            .arg("--preload")
            .args(["--threads", &threads])
            .env("PARAKEET_DEVICE", "cpu")
            .stdin(Stdio::piped())
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let child = command.spawn().map_err(|error| {
            let _ = fs::remove_file(&output_path);
            format!("No se pudo anticipar Parakeet Ultra: {error}")
        })?;
        Ok(Self { child, output_path })
    }

    fn finish(mut self, app: &AppHandle, input: &Path, language: &str, audio_seconds: f64)
        -> Result<TranscriptionResult, String> {
        let started = Instant::now();
        let mut stdin = self.child.stdin.take().ok_or("Parakeet anticipado no aceptó audio")?;
        let mut wav = File::open(input).map_err(|error| error.to_string())?;
        std::io::copy(&mut wav, &mut stdin).map_err(|error| format!("No se pudo enviar audio a Parakeet: {error}"))?;
        drop(stdin);
        let timeout = Duration::from_secs(180);
        let status = loop {
            if CANCEL_REQUESTED.load(Ordering::Acquire) {
                return Err("Transcripción cancelada".into());
            }
            match self.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(100)),
                Ok(None) => return Err("Parakeet superó tres minutos y se detuvo".into()),
                Err(error) => return Err(format!("No se pudo supervisar Parakeet: {error}")),
            }
        };
        if !status.success() {
            return Err(format!("Parakeet anticipado terminó con el código {}", status.code().unwrap_or(-1)));
        }
        let output = fs::read_to_string(&self.output_path)
            .map_err(|_| "No se pudo leer el resultado de Parakeet".to_string())?;
        let raw: ParakeetOutput = serde_json::from_str(output.trim())
            .map_err(|_| "Parakeet anticipado devolvió una respuesta inválida".to_string())?;
        result_from_raw_for_engine(app, &raw.text, language, audio_seconds,
            started.elapsed().as_millis(), TranscriptionEngine::ParakeetUltra)
    }
}

impl Drop for ParakeetPreload {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.output_path);
    }
}

fn run_parakeet(
    app: &AppHandle,
    input: &Path,
    language: &str,
    audio_seconds: f64,
) -> Result<TranscriptionResult, String> {
    if !matches!(language, "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    let (model, executable, vad) = verify_parakeet_resources(app)?;
    let output_path = input.with_file_name(format!(
        "parakeet-output-{}-{}.tmp",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos()
    ));
    let output_file = File::create(&output_path)
        .map_err(|_| "No se pudo crear la salida temporal de Parakeet".to_string())?;
    let _output_guard = TemporaryAudio(output_path.clone());
    let started = Instant::now();
    let mut command = Command::new(&executable);
    command.current_dir(executable.parent().ok_or("Ruta del motor Parakeet inválida")?)
        .args(["transcribe", "--model"])
        .arg(&model)
        .arg("--input")
        .arg(input)
        .args(["--json", "--vad", "--vad-model"])
        .arg(&vad)
        .env("PARAKEET_DEVICE", "cpu")
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn()
        .map_err(|_| "No se pudo iniciar Parakeet Ultra".to_string())?;
    let timeout = Duration::from_secs(180);
    let status = loop {
        if CANCEL_REQUESTED.load(Ordering::Acquire) {
            let _ = child.kill(); let _ = child.wait();
            return Err("Transcripción cancelada".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill(); let _ = child.wait();
                return Err("Parakeet superó tres minutos y se detuvo; prueba una grabación más corta".into());
            }
            Err(_) => {
                let _ = child.kill(); let _ = child.wait();
                return Err("No se pudo supervisar Parakeet Ultra".into());
            }
        }
    };
    if !status.success() {
        return Err(format!("Parakeet Ultra terminó con el código {}", status.code().unwrap_or(-1)));
    }
    let output = fs::read_to_string(&output_path)
        .map_err(|_| "No se pudo leer el resultado de Parakeet".to_string())?;
    let raw: ParakeetOutput = serde_json::from_str(output.trim())
        .map_err(|_| "Parakeet devolvió una respuesta inválida".to_string())?;
    result_from_raw_for_engine(app, &raw.text, language, audio_seconds, started.elapsed().as_millis(), TranscriptionEngine::ParakeetUltra)
}

fn run_whisper(
    app: &AppHandle,
    input: &Path,
    language: &str,
    audio_seconds: f64,
) -> Result<TranscriptionResult, String> {
    if !matches!(language, "auto" | "es" | "en") {
        return Err("El idioma seleccionado no es válido".into());
    }
    let (model, engine) = resources(app)?;
    if !engine.is_file() || !model.is_file() {
        return Err("El motor o el modelo local no están instalados".into());
    }
    let (_, vad) = verified_model_vad(app)?;
    let output_path = input.with_file_name(format!(
        "whisper-output-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    let output_file = File::create(&output_path)
        .map_err(|_| "No se pudo crear la salida temporal de transcripción".to_string())?;
    let _output_guard = TemporaryAudio(output_path.clone());
    let start = Instant::now();
    let mut command = Command::new(&engine);
    command
        .current_dir(engine.parent().ok_or("La ruta del motor es inválida")?)
        .args(["-m", model.to_str().ok_or("Ruta de modelo inválida")?])
        .args(["-f", input.to_str().ok_or("Ruta de audio inválida")?])
        .args(["-l", language, "-nt", "-np", "-ng", "-sns"])
        .args([
            "--vad",
            "-vm",
            vad.to_str().ok_or("Ruta del detector de voz inválida")?,
        ]);
    command.args(["--prompt", &initial_prompt(app, language)]);
    let mut child = command
        .creation_flags(0x08000000)
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "No se pudo iniciar el motor de transcripción local".to_string())?;
    let timeout = Duration::from_secs(180);
    let status = loop {
        if CANCEL_REQUESTED.load(Ordering::Acquire) {
            let _ = child.kill(); let _ = child.wait();
            return Err("Transcripción cancelada".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("La transcripción superó tres minutos y se detuvo; prueba una grabación más corta".into());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("No se pudo supervisar el motor de transcripción".into());
            }
        }
    };
    if !status.success() {
        return Err(format!(
            "El motor local terminó con el código {}",
            status.code().unwrap_or(-1)
        ));
    }
    let raw_text = fs::read_to_string(&output_path)
        .map_err(|_| "No se pudo leer la salida del motor local".to_string())?
        .trim()
        .to_string();
    if raw_text.is_empty() {
        return Err(
            "No se detectó voz en esta grabación. Revisa el micrófono e intenta hablar más cerca"
                .into(),
        );
    }
    result_from_raw(app, &raw_text, language, audio_seconds, start.elapsed().as_millis())
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;
