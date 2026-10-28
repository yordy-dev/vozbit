//! Anticipatory transcription. Audio is never inserted until the shortcut is released.
//! A short-lived loopback worker keeps one Whisper model loaded for this dictation.
use crate::{resample, transcription};
use crate::streaming_core::{pcm_wav, rms, AudioChunk, SegmentPlanner};
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const SERVER_SHA256: &str = "a97456e747f28e7bb09c1dac35f40d94404c5ecffa74aaaaa7a3753ea6543121";
const BLOCK_MILLIS: usize = 500;

pub(crate) struct AudioStreamSink {
    sender: SyncSender<AudioChunk>,
    overflow: Arc<AtomicBool>,
    pending: Vec<f32>,
    sample_rate: u32,
}

impl AudioStreamSink {
    pub(crate) fn push(&mut self, samples: &[f32], sample_rate: u32) {
        if samples.is_empty() || self.overflow.load(Ordering::Acquire) {
            return;
        }
        if self.sample_rate == 0 {
            self.sample_rate = sample_rate;
        } else if self.sample_rate != sample_rate {
            self.overflow.store(true, Ordering::Release);
            return;
        }
        self.pending.extend_from_slice(samples);
        let block = (sample_rate as usize * BLOCK_MILLIS / 1_000).max(1);
        while self.pending.len() >= block {
            let chunk = self.pending.drain(..block).collect();
            if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = self.sender.try_send(AudioChunk {
                sample_rate,
                samples: chunk,
            }) {
                self.overflow.store(true, Ordering::Release);
                return;
            }
        }
    }

    pub(crate) fn finish(&mut self) {
        if self.pending.is_empty() || self.overflow.load(Ordering::Acquire) {
            return;
        }
        let chunk = AudioChunk {
            sample_rate: self.sample_rate,
            samples: std::mem::take(&mut self.pending),
        };
        if self.sender.try_send(chunk).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
}

#[derive(Default, Clone)]
struct ServerControl(Arc<Mutex<Option<Child>>>);

impl ServerControl {
    fn stop(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn exited(&self) -> bool {
        self.0.lock().ok().and_then(|mut guard| guard.as_mut().and_then(|child| child.try_wait().ok().flatten())).is_some()
    }
}

struct ServerLifetime(ServerControl);

impl Drop for ServerLifetime {
    fn drop(&mut self) { self.0.stop(); }
}

pub(crate) struct StreamSession {
    sender: Option<SyncSender<AudioChunk>>,
    worker: Option<JoinHandle<Result<(StreamResult, usize), String>>>,
    overflow: Arc<AtomicBool>,
    aborted: Arc<AtomicBool>,
    control: ServerControl,
    prefetched: Arc<AtomicU32>,
}

pub(crate) struct StreamResult {
    pub(crate) raw_text: String,
    pub(crate) engine_ms: u128,
}

impl StreamSession {
    pub(crate) fn start(app: AppHandle, language: String) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(32);
        let overflow = Arc::new(AtomicBool::new(false));
        let aborted = Arc::new(AtomicBool::new(false));
        let control = ServerControl::default();
        let prefetched = Arc::new(AtomicU32::new(0));
        let worker_abort = aborted.clone();
        let worker_control = control.clone();
        let worker_prefetched = prefetched.clone();
        let worker = thread::Builder::new()
            .name("voicetext-anticipatory-asr".into())
            .spawn(move || run(app, language, receiver, worker_abort, worker_control, worker_prefetched))
            .map_err(|error| format!("No se pudo iniciar el procesamiento anticipado: {error}"))?;
        Ok(Self { sender: Some(sender), worker: Some(worker), overflow, aborted, control, prefetched })
    }

    pub(crate) fn sink(&self) -> AudioStreamSink {
        AudioStreamSink {
            sender: self.sender.as_ref().expect("sesión activa").clone(),
            overflow: self.overflow.clone(),
            pending: Vec::new(),
            sample_rate: 0,
        }
    }

    pub(crate) fn completed_before_stop(&self) -> u32 {
        self.prefetched.load(Ordering::Acquire)
    }

    pub(crate) fn finish(mut self, expected_frames: usize) -> Result<StreamResult, String> {
        self.sender.take();
        if self.overflow.load(Ordering::Acquire) {
            self.aborted.store(true, Ordering::Release);
            self.control.stop();
        }
        let result = self.worker.take().ok_or("No se encontró el procesamiento anticipado")?
            .join().map_err(|_| "El procesamiento anticipado terminó inesperadamente".to_string())?;
        self.control.stop();
        if self.overflow.load(Ordering::Acquire) {
            return Err("Se saturó la vista previa del audio; se usará el audio completo".into());
        }
        let result = result?;
        if result.1 != expected_frames {
            return Err("La vista previa no contiene todas las muestras; se usará el audio completo".into());
        }
        Ok(result.0)
    }

    pub(crate) fn cancel(mut self) {
        self.aborted.store(true, Ordering::Release);
        self.control.stop();
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for StreamSession {
    fn drop(&mut self) {
        self.aborted.store(true, Ordering::Release);
        self.control.stop();
        self.sender.take();
    }
}

fn run(
    app: AppHandle,
    language: String,
    receiver: Receiver<AudioChunk>,
    aborted: Arc<AtomicBool>,
    control: ServerControl,
    prefetched: Arc<AtomicU32>,
) -> Result<(StreamResult, usize), String> {
    let (port, prefix) = start_server(&app, &language, &aborted, &control)?;
    let _server_lifetime = ServerLifetime(control.clone());
    let client = Client::builder().no_proxy().timeout(Duration::from_secs(90)).build()
        .map_err(|error| format!("No se pudo preparar el cliente local: {error}"))?;
    let url = format!("http://127.0.0.1:{port}{prefix}/inference");
    let prompt = transcription::initial_prompt(&app, &language);
    let mut planner = SegmentPlanner::default();
    let mut texts = Vec::new();
    let mut engine_ms = 0;
    let mut frames = 0;
    while let Ok(chunk) = receiver.recv() {
        if aborted.load(Ordering::Acquire) {
            return Err("Procesamiento anticipado cancelado".into());
        }
        frames += chunk.samples.len();
        if let Some(segment) = planner.push(chunk)? {
            let (text, ms) = transcribe_segment(&client, &url, &language, &prompt, &segment, planner.sample_rate)?;
            if !text.is_empty() {
                texts.push(text);
            }
            engine_ms += ms;
            prefetched.fetch_add(1, Ordering::Release);
        }
    }
    if aborted.load(Ordering::Acquire) {
        return Err("Procesamiento anticipado cancelado".into());
    }
    if let Some(tail) = planner.finish() {
        let (text, ms) = transcribe_segment(&client, &url, &language, &prompt, &tail, planner.sample_rate)?;
        if !text.is_empty() {
            texts.push(text);
        }
        engine_ms += ms;
    }
    let raw_text = texts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    if raw_text.is_empty() {
        return Err("El servidor local no reconoció voz; se usará el audio completo".into());
    }
    let result = StreamResult { raw_text, engine_ms };
    Ok((result, frames))
}

#[derive(Deserialize)]
struct InferenceResponse { text: String }

fn transcribe_segment(
    client: &Client,
    url: &str,
    language: &str,
    prompt: &str,
    input: &[f32],
    input_rate: u32,
) -> Result<(String, u128), String> {
    let samples = resample::to_16khz(input, input_rate)?;
    if samples.len() < 16_000 || rms(&samples) < 0.005 {
        return Ok((String::new(), 0));
    }
    let wav = pcm_wav(&samples)?;
    let part = multipart::Part::bytes(wav).file_name("segment.wav")
        .mime_str("audio/wav").map_err(|error| error.to_string())?;
    let form = multipart::Form::new()
        .text("language", language.to_string())
        .text("response_format", "json")
        .text("prompt", prompt.to_string())
        .part("file", part);
    let started = Instant::now();
    let response = client.post(url).multipart(form).send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Falló el motor local de fragmentos: {error}"))?;
    let body: InferenceResponse = response.json()
        .map_err(|error| format!("El motor local devolvió una respuesta inválida: {error}"))?;
    let text = body.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() && rms(&samples) >= 0.005 {
        return Err("El motor local omitió un fragmento con voz; se usará el audio completo".into());
    }
    Ok((text, started.elapsed().as_millis()))
}

fn start_server(
    app: &AppHandle,
    language: &str,
    aborted: &AtomicBool,
    control: &ServerControl,
) -> Result<(u16, String), String> {
    use tauri::path::BaseDirectory;
    let (model, vad) = transcription::verified_model_vad(app)?;
    let executable = app.path().resolve("resources/whisper/whisper-server.exe", BaseDirectory::Resource)
        .map_err(|error| error.to_string())?;
    if transcription::sha256(&executable)? != SERVER_SHA256 {
        return Err("El servidor local no pasó la comprobación de integridad".into());
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("No se pudo reservar un puerto local: {error}"))?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    drop(listener);
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?.as_nanos();
    let prefix = format!("/vt-{}-{nonce}", std::process::id());
    let mut command = Command::new(&executable);
    command.current_dir(executable.parent().ok_or("Ruta del servidor inválida")?)
        .args(["-m", model.to_str().ok_or("Ruta de modelo inválida")?])
        .args(["-l", language, "-ng", "-sns", "-nt", "-bs", "5", "-bo", "5"])
        .args(["--vad", "-vm", vad.to_str().ok_or("Ruta de VAD inválida")?])
        .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--request-path", &prefix])
        .stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let child = command.spawn().map_err(|error| format!("No se pudo iniciar el servidor local: {error}"))?;
    *control.0.lock().map_err(|_| "No se pudo supervisar el servidor local")? = Some(child);
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    for _ in 0..100 {
        if aborted.load(Ordering::Acquire) { control.stop(); return Err("Dictado cancelado".into()); }
        if control.exited() { control.stop(); return Err("El servidor local terminó al iniciar".into()); }
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            return Ok((port, prefix));
        }
        thread::sleep(Duration::from_millis(50));
    }
    control.stop();
    Err("El servidor local no respondió en cinco segundos".into())
}

