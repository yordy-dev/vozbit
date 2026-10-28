use crate::resample;
use crate::streaming::AudioStreamSink;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const TEST_DURATION: Duration = Duration::from_secs(5);
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

struct CaptureGuard;

impl CaptureGuard {
    fn acquire() -> Result<Self, String> {
        CAPTURE_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Ya hay una captura de micrófono en curso".to_string())?;
        Ok(Self)
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        CAPTURE_ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
pub struct AudioDevice {
    id: String,
    name: String,
    is_default: bool,
    sample_rate: u32,
    channels: u16,
    sample_format: String,
}

#[derive(Clone, Serialize)]
struct MicLevel {
    rms: f32,
    peak: f32,
}

#[derive(Serialize)]
pub struct AudioTestResult {
    device_name: String,
    input_sample_rate: u32,
    output_sample_rate: u32,
    channels: u16,
    sample_format: String,
    pub(crate) input_frames: usize,
    output_frames: usize,
    duration_seconds: f64,
    rms: f32,
    peak: f32,
    pub(crate) dropped_chunks: u32,
    saved_path: Option<String>,
}

#[tauri::command]
pub fn audio_list_input_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let devices = host
        .input_devices()
        .map_err(|error| format!("Windows no pudo enumerar los micrófonos: {error}"))?;
    let mut result = Vec::new();
    for device in devices {
        let Ok(id) = device.id() else { continue };
        let Ok(config) = device.default_input_config() else {
            continue;
        };
        let name = device
            .description()
            .map(|description| description.name().to_string())
            .unwrap_or_else(|_| "Micrófono sin nombre".to_string());
        let id = id.to_string();
        result.push(AudioDevice {
            is_default: default_id.as_ref() == Some(&id),
            id,
            name,
            sample_rate: config.sample_rate(),
            channels: config.channels(),
            sample_format: config.sample_format().to_string(),
        });
    }
    result.sort_by_key(|item| !item.is_default);
    Ok(result)
}

#[tauri::command]
pub async fn audio_test_input_device(
    app: AppHandle,
    device_id: Option<String>,
    save_wav: bool,
) -> Result<AudioTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_test(app, device_id, save_wav))
        .await
        .map_err(|error| format!("La tarea de audio terminó inesperadamente: {error}"))?
}

fn run_test(
    app: AppHandle,
    device_id: Option<String>,
    save_wav: bool,
) -> Result<AudioTestResult, String> {
    let mut captured = capture_input(
        &app,
        device_id,
        TEST_DURATION,
        Arc::new(AtomicBool::new(false)),
        None,
        None,
    )?;
    if save_wav {
        captured.result.saved_path = Some(write_diagnostic_wav(&app, &captured.samples)?);
    }
    Ok(captured.result)
}

pub(crate) struct CapturedInput {
    pub(crate) result: AudioTestResult,
    pub(crate) samples: Vec<f32>,
}

pub(crate) fn capture_input(
    app: &AppHandle,
    device_id: Option<String>,
    max_duration: Duration,
    stop: Arc<AtomicBool>,
    ready: Option<SyncSender<Result<(), String>>>,
    mut prefetch: Option<AudioStreamSink>,
) -> Result<CapturedInput, String> {
    let _guard = CaptureGuard::acquire()?;
    let host = cpal::default_host();
    let device = if let Some(id) = device_id {
        host.input_devices()
            .map_err(|error| error.to_string())?
            .find(|candidate| {
                candidate
                    .id()
                    .is_ok_and(|candidate_id| candidate_id.to_string() == id)
            })
            .ok_or_else(|| "El micrófono seleccionado ya no está disponible".to_string())?
    } else {
        host.default_input_device()
            .ok_or_else(|| "Windows no encontró un micrófono predeterminado".to_string())?
    };
    let device_name = device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|_| "Micrófono sin nombre".to_string());
    let supported = device
        .default_input_config()
        .map_err(|error| format!("El micrófono no ofrece un formato de entrada: {error}"))?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels;
    let input_rate = config.sample_rate;
    if channels == 0 || input_rate == 0 {
        return Err("El micrófono informó una configuración de audio inválida".into());
    }

    let (sample_tx, sample_rx) = mpsc::sync_channel::<Vec<f32>>(64);
    let (error_tx, error_rx) = mpsc::sync_channel::<String>(2);
    let dropped = Arc::new(AtomicU32::new(0));
    let stream = match sample_format {
        SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::F64 => {
            build_stream::<f64>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::I8 => {
            build_stream::<i8>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::I24 => {
            build_stream::<cpal::I24>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::I32 => {
            build_stream::<i32>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::I64 => {
            build_stream::<i64>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::U8 => {
            build_stream::<u8>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::U16 => {
            build_stream::<u16>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::U24 => {
            build_stream::<cpal::U24>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::U32 => {
            build_stream::<u32>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        SampleFormat::U64 => {
            build_stream::<u64>(&device, &config, sample_tx, error_tx, dropped.clone())
        }
        _ => Err(format!(
            "El formato de micrófono {sample_format} no es PCM compatible"
        )),
    }?;
    stream
        .play()
        .map_err(|error| format!("Windows no permitió iniciar la captura: {error}"))?;
    if let Some(sender) = ready {
        let _ = sender.try_send(Ok(()));
    }

    let start = Instant::now();
    let mut samples = Vec::with_capacity(input_rate as usize * 5);
    let mut peak = 0.0_f32;
    let mut sum_squares = 0.0_f64;
    let mut last_level = Instant::now() - Duration::from_millis(100);
    while start.elapsed() < max_duration && !stop.load(Ordering::Acquire) {
        if let Ok(error) = error_rx.try_recv() {
            return Err(format!("El micrófono interrumpió la captura: {error}"));
        }
        if let Ok(chunk) = sample_rx.recv_timeout(Duration::from_millis(50)) {
            let first = samples.len();
            let mut chunk_peak = 0.0_f32;
            let mut chunk_squares = 0.0_f64;
            for &sample in &chunk {
                let safe = if sample.is_finite() {
                    sample.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                chunk_peak = chunk_peak.max(safe.abs());
                chunk_squares += (safe as f64) * (safe as f64);
                samples.push(safe);
            }
            if let Some(sink) = &mut prefetch {
                sink.push(&samples[first..], input_rate);
            }
            peak = peak.max(chunk_peak);
            sum_squares += chunk_squares;
            if last_level.elapsed() >= Duration::from_millis(80) && !chunk.is_empty() {
                let _ = app.emit(
                    "audio://mic-level",
                    MicLevel {
                        rms: (chunk_squares / chunk.len() as f64).sqrt() as f32,
                        peak: chunk_peak,
                    },
                );
                last_level = Instant::now();
            }
        }
    }
    drop(stream);
    for chunk in sample_rx.try_iter() {
        let first = samples.len();
        for sample in chunk {
            let safe = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            peak = peak.max(safe.abs());
            sum_squares += (safe as f64) * (safe as f64);
            samples.push(safe);
        }
        if let Some(sink) = &mut prefetch {
            sink.push(&samples[first..], input_rate);
        }
    }
    if let Some(sink) = &mut prefetch { sink.finish(); }
    if samples.is_empty() {
        return Err("No llegaron muestras. Comprueba el permiso de micrófono de Windows".into());
    }
    let rms = (sum_squares / samples.len() as f64).sqrt() as f32;
    let input_frames = samples.len();
    let converted = resample::to_16khz(&samples, input_rate)?;
    Ok(CapturedInput {
        result: AudioTestResult {
            device_name,
            input_sample_rate: input_rate,
            output_sample_rate: 16_000,
            channels,
            sample_format: sample_format.to_string(),
            input_frames,
            output_frames: converted.len(),
            duration_seconds: input_frames as f64 / input_rate as f64,
            rms,
            peak,
            dropped_chunks: dropped.load(Ordering::Relaxed),
            saved_path: None,
        },
        samples: converted,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_tx: SyncSender<Vec<f32>>,
    error_tx: SyncSender<String>,
    dropped: Arc<AtomicU32>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Copy,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mono: Vec<f32> = data
                    .chunks_exact(channels)
                    .map(|frame| {
                        frame
                            .iter()
                            .map(|&sample| f32::from_sample(sample))
                            .sum::<f32>()
                            / channels as f32
                    })
                    .collect();
                if let Err(TrySendError::Full(_)) = sample_tx.try_send(mono) {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            },
            move |error| {
                let _ = error_tx.try_send(error.to_string());
            },
            None,
        )
        .map_err(|error| format!("No se pudo abrir el micrófono: {error}"))
}

pub(crate) fn diagnostic_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let path = match (
        cfg!(debug_assertions),
        std::env::var_os("VOICETEXT_DIAGNOSTICS_DIR"),
    ) {
        (true, Some(value)) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err("La ruta de diagnóstico de desarrollo debe ser absoluta".into());
            }
            path
        }
        _ => app
            .path()
            .app_data_dir()
            .map_err(|error| format!("No se encontró la carpeta de datos: {error}"))?
            .join("diagnostics"),
    };
    Ok(path)
}

fn write_diagnostic_wav(app: &AppHandle, samples: &[f32]) -> Result<String, String> {
    let directory = diagnostic_dir(app)?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("No se pudo crear la carpeta de diagnóstico: {error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = directory.join(format!("mic-test-{timestamp}.wav"));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec)
        .map_err(|error| format!("No se pudo crear el WAV: {error}"))?;
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer
            .write_sample(value)
            .map_err(|error| format!("No se pudo escribir el WAV: {error}"))?;
    }
    writer
        .finalize()
        .map_err(|error| format!("No se pudo cerrar el WAV: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}
