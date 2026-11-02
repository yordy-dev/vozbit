use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};
use tauri::{AppHandle, Manager};

const MIN_TRANSCRIPTION_SPACE_BYTES: u64 = 128 * 1024 * 1024;
const LOW_SPACE_WARNING_BYTES: u64 = 1024 * 1024 * 1024;
const LOW_MEMORY_WARNING_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct DiagnosticCheck {
    id: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Clone, Serialize)]
pub struct DiagnosticReport {
    app_version: String,
    architecture: &'static str,
    os_version: String,
    logical_processors: usize,
    total_memory_bytes: u64,
    available_memory_bytes: u64,
    data_available_space_bytes: Option<u64>,
    cache_available_space_bytes: Option<u64>,
    cpu_features: Vec<&'static str>,
    stale_files_removed: usize,
    checks: Vec<DiagnosticCheck>,
}

fn check(id: &'static str, result: Result<String, String>) -> DiagnosticCheck {
    match result {
        Ok(detail) => DiagnosticCheck {
            id,
            status: "ok",
            detail,
        },
        Err(detail) => DiagnosticCheck {
            id,
            status: "error",
            detail,
        },
    }
}

fn warning(id: &'static str, detail: String) -> DiagnosticCheck {
    DiagnosticCheck {
        id,
        status: "warning",
        detail,
    }
}

fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut candidate = path.to_path_buf();
    loop {
        if candidate.exists() {
            return Some(candidate.canonicalize().unwrap_or(candidate));
        }
        if !candidate.pop() {
            return None;
        }
    }
}

pub(crate) fn available_space(path: &Path) -> Option<u64> {
    let existing = existing_ancestor(path)?;
    let normalized = normalize_windows_path(&existing);
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter_map(|disk| {
            let mount = normalize_windows_path(disk.mount_point());
            normalized
                .starts_with(&mount)
                .then_some((mount.len(), disk.available_space()))
        })
        .max_by_key(|(mount_length, _)| *mount_length)
        .map(|(_, bytes)| bytes)
}

fn normalize_windows_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(rest) = normalized.strip_prefix(r"\\?\unc\") {
        format!(r"\\{rest}")
    } else {
        normalized
            .strip_prefix(r"\\?\")
            .unwrap_or(&normalized)
            .to_string()
    }
}

fn cpu_features() -> Vec<&'static str> {
    let mut features = Vec::new();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sse4.1") {
            features.push("sse4.1");
        }
        if std::is_x86_feature_detected!("avx") {
            features.push("avx");
        }
        if std::is_x86_feature_detected!("avx2") {
            features.push("avx2");
        }
        if std::is_x86_feature_detected!("fma") {
            features.push("fma");
        }
    }
    features
}

pub(crate) fn ensure_transcription_capacity(app: &AppHandle) -> Result<(), String> {
    let cache_directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("No se encontró la carpeta temporal: {error}"))?
        .join("transcription");
    fs::create_dir_all(&cache_directory)
        .map_err(|error| format!("No se pudo preparar la carpeta temporal: {error}"))?;
    if let Some(bytes) = available_space(&cache_directory) {
        if bytes < MIN_TRANSCRIPTION_SPACE_BYTES {
            return Err(format!(
                "No hay espacio temporal suficiente para transcribir (quedan {} MiB; libera al menos 128 MiB)",
                bytes / 1024 / 1024
            ));
        }
    }
    Ok(())
}

fn writable_directory(path: &std::path::Path) -> Result<String, String> {
    fs::create_dir_all(path).map_err(|error| format!("No se pudo crear la carpeta: {error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let probe = path.join(format!(".vozbit-write-{}-{stamp}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
        .map_err(|error| format!("No se puede escribir en la carpeta: {error}"))?;
    file.write_all(b"vozbit")
        .map_err(|error| format!("No se pudo completar la prueba: {error}"))?;
    drop(file);
    fs::remove_file(&probe)
        .map_err(|error| format!("No se pudo limpiar el archivo de prueba: {error}"))?;
    Ok("Lectura y escritura disponibles".into())
}

fn build_report(app: &AppHandle) -> Result<DiagnosticReport, String> {
    let data_directory = crate::storage::database_path(app)?
        .parent()
        .ok_or("La ruta de datos no tiene carpeta padre")?
        .to_path_buf();
    let cache_directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("No se encontró la carpeta temporal: {error}"))?
        .join("transcription");
    let stale_files_removed = crate::transcription::cleanup_stale_files(app).unwrap_or(0);
    let data_available_space_bytes = available_space(&data_directory);
    let cache_available_space_bytes = available_space(&cache_directory);
    let mut system = System::new();
    system.refresh_memory();
    let total_memory_bytes = system.total_memory();
    let available_memory_bytes = system.available_memory();
    let mut checks = vec![
        check(
            "database",
            crate::storage::database_quick_check(app).map(|_| "Base local íntegra".into()),
        ),
        check("data_directory", writable_directory(&data_directory)),
        check("cache_directory", writable_directory(&cache_directory)),
    ];
    if available_memory_bytes < LOW_MEMORY_WARNING_BYTES {
        checks.push(warning(
                "memory",
                format!(
                    "Memoria disponible baja: {} MiB. Cierra otras aplicaciones antes de usar Parakeet.",
                    available_memory_bytes / 1024 / 1024
                ),
            ));
    } else {
        checks.push(check(
            "memory",
            Ok(format!(
                "{} MiB disponibles de {} MiB",
                available_memory_bytes / 1024 / 1024,
                total_memory_bytes / 1024 / 1024
            )),
        ));
    }
    for (id, label, bytes) in [
        ("data_space", "datos", data_available_space_bytes),
        ("cache_space", "temporales", cache_available_space_bytes),
    ] {
        checks.push(match bytes {
            Some(value) if value < LOW_SPACE_WARNING_BYTES => warning(
                id,
                format!(
                    "Espacio bajo para {label}: {} MiB disponibles",
                    value / 1024 / 1024
                ),
            ),
            Some(value) => check(
                id,
                Ok(format!(
                    "{} MiB disponibles para {label}",
                    value / 1024 / 1024
                )),
            ),
            None => warning(id, format!("No se pudo medir el espacio para {label}")),
        });
    }
    Ok(DiagnosticReport {
        app_version: app.package_info().version.to_string(),
        architecture: std::env::consts::ARCH,
        os_version: System::long_os_version().unwrap_or_else(|| "Windows no identificado".into()),
        logical_processors: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
        total_memory_bytes,
        available_memory_bytes,
        data_available_space_bytes,
        cache_available_space_bytes,
        cpu_features: cpu_features(),
        stale_files_removed,
        checks,
    })
}

#[tauri::command]
pub async fn diagnostics_run(app: AppHandle) -> Result<DiagnosticReport, String> {
    tauri::async_runtime::spawn_blocking(move || build_report(&app))
        .await
        .map_err(|error| format!("El diagnóstico terminó inesperadamente: {error}"))?
}

#[tauri::command]
pub async fn diagnostics_export(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let report = build_report(&app)?;
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let directory = crate::storage::database_path(&app)?
            .parent()
            .ok_or("La ruta de datos no tiene carpeta padre")?
            .join("diagnostics");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("No se pudo crear la carpeta de diagnósticos: {error}"))?;
        let path = directory.join(format!("vozbit-diagnostics-{created_at_ms}.json"));
        let bytes = serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("No se pudo preparar el diagnóstico: {error}"))?;
        fs::write(&path, bytes)
            .map_err(|error| format!("No se pudo guardar el diagnóstico: {error}"))?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| format!("La exportación del diagnóstico terminó inesperadamente: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::normalize_windows_path;
    use std::path::Path;

    #[test]
    fn normalizes_extended_windows_paths_for_disk_matching() {
        assert_eq!(
            normalize_windows_path(Path::new(r"\\?\C:\Users\Test")),
            r"c:\users\test"
        );
        assert_eq!(
            normalize_windows_path(Path::new(r"\\?\UNC\Server\Share\Folder")),
            r"\\server\share\folder"
        );
    }
}
