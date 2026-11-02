import { access } from "node:fs/promises";
import { resolve, sep } from "node:path";

const port = process.argv[2] ?? "9460";
const isolatedDataDirectory = process.argv[3] ? resolve(process.argv[3]) : null;
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find((page) => [
  "http://tauri.localhost/",
  "http://localhost:5173/",
  "http://127.0.0.1:5173/",
].includes(page.url));
if (!main) throw Error("No se encontró la ventana principal");

const socket = new WebSocket(main.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.addEventListener("open", resolve, { once: true });
  socket.addEventListener("error", reject, { once: true });
});

let nextId = 0;
const pending = new Map();
socket.addEventListener("message", ({ data }) => {
  const message = JSON.parse(data);
  const task = pending.get(message.id);
  if (!task) return;
  pending.delete(message.id);
  message.error ? task.reject(Error(message.error.message)) : task.resolve(message.result);
});

function send(method, params = {}) {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}

async function evaluate(expression) {
  const response = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    const detail = response.exceptionDetails.exception?.description ?? response.exceptionDetails.text;
    throw Error(detail);
  }
  return response.result.value;
}

async function invoke(command, args = {}) {
  return evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})
    .then(value => ({ ok: true, value }), error => ({ ok: false, error: String(error) }))`);
}

function assert(value, message) {
  if (!value) throw Error(message);
}

let originalPrivateMode;
try {
  await evaluate("document.querySelector('button[aria-label=\"Omitir tutorial\"]')?.click()");
  const settings = await invoke("settings_get");
  assert(settings.ok, `No se pudo leer la configuración: ${settings.error}`);
  originalPrivateMode = Boolean(settings.value.private_mode);

  const diagnostics = await invoke("diagnostics_run");
  assert(diagnostics.ok, `Falló el diagnóstico: ${diagnostics.error}`);
  assert(diagnostics.value.checks.every((item) => item.status !== "error"), "El diagnóstico detectó errores locales");

  const privateOn = await invoke("settings_patch", { patch: { private_mode: true } });
  assert(privateOn.ok, `No se pudo activar el modo privado: ${privateOn.error}`);
  const privateSettings = await invoke("settings_get");
  assert(privateSettings.value.private_mode === true, "El modo privado no quedó activo");

  const forbiddenSave = await invoke("history_save", {
    text: "No debe persistirse",
    language: "es",
    audioSeconds: 1,
  });
  assert(!forbiddenSave.ok && forbiddenSave.error.includes("modo privado"), "El modo privado permitió guardar historial");

  const forget = await invoke("dictation_forget_audio");
  assert(forget.ok, `No se pudo borrar el audio temporal: ${forget.error}`);

  let backup = null;
  let clearedNonFavorites = null;
  let diagnosticExport = null;
  if (isolatedDataDirectory) {
    const exported = await invoke("diagnostics_export");
    assert(exported.ok, `No se pudo exportar el diagnóstico: ${exported.error}`);
    diagnosticExport = resolve(exported.value);
    assert(diagnosticExport.startsWith(`${isolatedDataDirectory}${sep}`), "El diagnóstico no pertenece al perfil aislado");
    await access(diagnosticExport);

    const privateOff = await invoke("settings_patch", { patch: { private_mode: false } });
    assert(privateOff.ok, `No se pudo preparar el historial aislado: ${privateOff.error}`);
    const marker = `smoke-hardening-${Date.now()}`;
    const seeded = await invoke("history_save", { text: marker, language: "es", audioSeconds: 1 });
    assert(seeded.ok && Number.isInteger(seeded.value), `No se pudo crear la entrada aislada: ${seeded.error}`);

    const backupResult = await invoke("data_backup_create");
    assert(backupResult.ok, `No se pudo crear el respaldo: ${backupResult.error}`);
    await access(backupResult.value.path);
    const normalizedBackup = resolve(backupResult.value.path);
    assert(normalizedBackup.startsWith(`${isolatedDataDirectory}${sep}`), "El respaldo no pertenece al perfil aislado; se canceló la prueba destructiva");
    backup = backupResult.value;

    const clear = await invoke("history_clear", { includeFavorites: false });
    assert(clear.ok && clear.value >= 1, `No se pudo limpiar el historial aislado: ${clear.error}`);
    const remaining = await invoke("history_list", { query: marker });
    assert(remaining.ok && remaining.value.length === 0, "La entrada de prueba no fue eliminada");
    clearedNonFavorites = clear.value;
  }

  console.log(JSON.stringify({
    diagnostics: diagnostics.value.checks,
    privateModeBlocksHistory: true,
    temporaryAudioForgotten: true,
    isolatedDataVerified: Boolean(isolatedDataDirectory),
    diagnosticExport,
    backup,
    clearedNonFavorites,
  }, null, 2));
} finally {
  if (originalPrivateMode !== undefined) {
    await invoke("settings_patch", { patch: { private_mode: originalPrivateMode } }).catch(() => undefined);
  }
  socket.close();
}
