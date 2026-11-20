import { readFile } from "node:fs/promises";

const files = Object.fromEntries(await Promise.all([
  "src-tauri/src/delivery.rs",
  "src-tauri/src/dictation.rs",
  "src-tauri/src/diagnostics.rs",
  "src-tauri/src/learning.rs",
  "src-tauri/src/storage.rs",
  "src-tauri/src/shortcut.rs",
  "src-tauri/src/lib.rs",
  "src/App.tsx",
  "src/SettingsPanel.tsx",
  "src/redesign.css",
  "src/overlay-refined.css",
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
assert(files["src/App.tsx"].includes("vt-home-actions"), "Las acciones de dictado no aparecen en Inicio");
assert(files["src/SettingsPanel.tsx"].includes("<Icon name={item.icon}"), "Las categorías de Configuración no tienen apoyo visual");
assert(files["src/App.tsx"].includes("data-theme={settings.theme}"), "El tema guardado no se aplica a la interfaz");
assert(["system", "light", "dark", "contrast"].every(theme => files["src/SettingsPanel.tsx"].includes(`[\"${theme}\"`)), "Faltan variantes de tema en Apariencia");
assert(files["src-tauri/src/storage.rs"].includes("El tema seleccionado no es válido"), "El backend no valida el tema antes de guardarlo");
assert(files["src-tauri/src/storage.rs"].includes("transcription_to_clipboard"), "Falta persistencia para la transcripción al portapapeles");
assert(files["src-tauri/src/shortcut.rs"].includes("ShortcutAction") && files["src-tauri/src/shortcut.rs"].includes("shortcut_action_update"), "Los atajos fijos no se pueden configurar por acción");
assert(files["src-tauri/src/shortcut.rs"].includes("Esa combinación ya se usa para"), "Los atajos configurables no detectan conflictos");
assert(files["src-tauri/src/shortcut.rs"].includes("crate::dictation::cancel_processing(app)"), "El atajo de cancelar no llega directamente al estado del dictado");
assert(files["src-tauri/src/lib.rs"].includes("tauri_plugin_clipboard_manager::init"), "Falta el gestor nativo del portapapeles");
assert(files["src/SettingsPanel.tsx"].includes("Transcripción a portapapeles"), "Falta el interruptor solicitado en Texto y salida");
assert(files["src/App.tsx"].includes("writeClipboardText(completed.transcript.text)"), "La transcripción terminada no se copia automáticamente");
assert(files["src/redesign.css"].includes("@media(forced-colors:active)"), "Falta soporte de colores forzados en la interfaz");
assert(files["src/redesign.css"].includes("@media(prefers-reduced-motion:reduce)"), "Falta soporte de movimiento reducido en la interfaz");
assert(!files["src/redesign.css"].includes(".overlay-shell"), "El rediseño de la app invade los estilos de la pastilla flotante");
assert(files["src/overlay-refined.css"].includes(".overlay-shell.state-processing"), "La pastilla perdió sus estados visuales");
assert(files["src/types.ts"].includes("code: string | null"), "El frontend no tipa el código de entrega");
assert(files["tests/smoke-hardening-v059.mjs"].includes("if (isolatedDataDirectory)"), "La prueba destructiva no exige un perfil aislado");
assert(files["tests/smoke-hardening-v059.mjs"].includes("se canceló la prueba destructiva"), "La prueba no verifica la ruta aislada antes de limpiar datos");

console.log("36 comprobaciones estáticas de endurecimiento aprobadas");
