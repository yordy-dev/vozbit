import { readFile } from "node:fs/promises";

const files = Object.fromEntries(await Promise.all([
  "src-tauri/src/delivery.rs",
  "src-tauri/src/dictation.rs",
  "src-tauri/src/diagnostics.rs",
  "src-tauri/src/learning.rs",
  "src-tauri/src/storage.rs",
  "src-tauri/src/lib.rs",
  "src/App.tsx",
  "src/SettingsPanel.tsx",
  "src/types.ts",
  "tests/smoke-hardening-v059.mjs",
].map(async path => [path, await readFile(new URL(`../${path}`, import.meta.url), "utf8")])));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

assert(files["src-tauri/src/delivery.rs"].includes("UIA_DocumentControlTypeId"), "La entrega no admite editores tipo documento");
assert(files["src-tauri/src/delivery.rs"].includes("FocusVerification::Matched => return Ok(())"), "UI Automation no tiene prioridad sobre señales efímeras");
assert(files["src-tauri/src/delivery.rs"].includes("code: Option<String>"), "Faltan códigos estables de entrega");
assert(files["src-tauri/src/dictation.rs"].includes("request_shutdown"), "Falta limpieza de sesión al salir");
assert(files["src-tauri/src/learning.rs"].includes("OBSERVATION_LIMIT: Duration = Duration::from_secs(30)"), "La observación no coincide con los 30 segundos documentados");
assert(files["src-tauri/src/learning.rs"].includes("OBSERVE_SHA256") && files["src-tauri/src/learning.rs"].includes("DIFF_SHA256"), "Los scripts de aprendizaje no verifican integridad");
assert(files["src-tauri/src/storage.rs"].includes("private_mode"), "Falta modo privado en almacenamiento");
assert(files["src-tauri/src/storage.rs"].includes("PRAGMA quick_check(1)"), "Falta comprobación de la base local");
assert(files["src-tauri/src/storage.rs"].includes("vozbit-backup-v1"), "Falta respaldo local versionado");
assert(files["src-tauri/src/diagnostics.rs"].includes("stale_files_removed"), "Falta limpieza observable de temporales");
assert(files["src-tauri/src/diagnostics.rs"].includes("ensure_transcription_capacity"), "Falta comprobación de espacio antes de transcribir");
assert(files["src-tauri/src/diagnostics.rs"].includes("available_memory_bytes") && files["src-tauri/src/diagnostics.rs"].includes("os_version"), "El diagnóstico no informa sistema y memoria");
assert(files["src-tauri/src/diagnostics.rs"].includes("diagnostics_export"), "Falta exportación anónima para soporte");
assert(files["src-tauri/src/lib.rs"].includes("tauri_plugin_single_instance::init"), "La app no limita instancias múltiples");
assert(files["src-tauri/src/lib.rs"].includes("database_quick_check(app.handle())"), "El arranque no comprueba la integridad de la base local");
assert(files["src/App.tsx"].includes("settings.auto_save && !settings.private_mode"), "El frontend puede guardar durante modo privado");
assert(files["src/SettingsPanel.tsx"].includes("Ejecutar diagnóstico") && files["src/SettingsPanel.tsx"].includes("Crear respaldo"), "Faltan acciones de diagnóstico o respaldo");
assert(files["src/types.ts"].includes("code: string | null"), "El frontend no tipa el código de entrega");
assert(files["tests/smoke-hardening-v059.mjs"].includes("if (isolatedDataDirectory)"), "La prueba destructiva no exige un perfil aislado");
assert(files["tests/smoke-hardening-v059.mjs"].includes("se canceló la prueba destructiva"), "La prueba no verifica la ruta aislada antes de limpiar datos");

console.log("20 comprobaciones estáticas de endurecimiento aprobadas");
