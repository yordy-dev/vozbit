import { execFileSync } from "node:child_process";

const port = process.argv[2] ?? "9472";
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find(page => ["http://tauri.localhost/", "http://localhost:5173/", "http://127.0.0.1:5173/"].includes(page.url));
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
  const response = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (response.exceptionDetails) throw Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
  return response.result.value;
}
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
const assert = (condition, message) => { if (!condition) throw Error(message); };
async function waitUntil(expression, message, attempts = 50) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (await evaluate(expression)) return;
    await wait(100);
  }
  throw Error(message);
}
const powershell = command => execFileSync("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", ["-NoProfile", "-Command", command], { encoding: "utf8" });
const originalClipboard = powershell("Get-Clipboard -Raw -ErrorAction SilentlyContinue");
const originalSettings = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
const originalShortcuts = await evaluate("window.__TAURI_INTERNALS__.invoke('shortcuts_status')");
const testShortcuts = {
  transcription: "Ctrl+Alt+Shift+F5",
  cancel: "Ctrl+Alt+Shift+F6",
  reprocess: "Ctrl+Alt+Shift+F7",
  learn_selected: "Ctrl+Alt+Shift+F8",
};

try {
  if (await evaluate("!!document.querySelector('.tour-close')")) {
    await evaluate("document.querySelector('.tour-close').click()");
    await waitUntil("!!document.querySelector('button[aria-label=\"Abrir Configuración\"]')", "No apareció Inicio después del tutorial");
  }
  if (!(await evaluate("!!document.querySelector('.vt-settings-layout')"))) {
    const opened = await evaluate("(() => { const button = document.querySelector('button[aria-label=\"Abrir Configuración\"]'); if (!button) return false; button.click(); return true; })()");
    if (!opened) throw Error(`No se encontró el botón para abrir Configuración. Estado: ${await evaluate("document.body.innerText.slice(0, 800)")}`);
    await waitUntil("!!document.querySelector('.vt-settings-layout')", "No se abrió Configuración");
  }
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'General')?.click()");
  await wait(150);
  const generalFields = await evaluate("document.querySelectorAll('.vt-settings-page:not([hidden]) .shortcut-editor input').length");
  assert(generalFields === 2, "General no muestra los atajos editables de transcripción y cancelación");
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Texto y salida')?.click()");
  await wait(150);
  assert(await evaluate("document.body.innerText.includes('Transcripción a portapapeles')"), "Falta el interruptor Transcripción a portapapeles");
  assert(await evaluate("document.querySelector('.vt-settings-page:not([hidden]) .shortcut-editor input[aria-label=\"Atajo para agregar palabra seleccionada\"]') !== null"), "El atajo del diccionario no es editable");
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Datos y app')?.click()");
  await wait(150);
  assert(await evaluate("document.querySelector('.vt-settings-page:not([hidden]) .shortcut-editor input[aria-label=\"Atajo para reprocesar último audio\"]') !== null"), "El atajo de reprocesado no es editable");

  for (const [action, shortcut] of Object.entries(testShortcuts)) {
    const status = await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action, shortcut })})`);
    assert(status[action].shortcut === shortcut && status[action].registered, `No se registró ${action}`);
  }
  const conflict = await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action: "cancel", shortcut: testShortcuts.transcription })}).then(() => null, error => String(error))`);
  assert(conflict?.includes("ya se usa"), "No se rechazó una combinación duplicada");

  await evaluate("window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { transcription_to_clipboard: true } })");
  const stored = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
  assert(stored.transcription_to_clipboard === true, "No se guardó el interruptor de portapapeles");
  const marker = `Vozbit portapapeles ${Date.now()}`;
  await evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:clipboard-manager|write_text', { text: ${JSON.stringify(marker)}, label: null })`);
  assert(powershell("Get-Clipboard -Raw").trimEnd() === marker, "El gestor nativo no escribió el texto esperado");

  console.log(JSON.stringify({ editableShortcuts: 4, conflictRejected: true, clipboardSettingPersisted: true, nativeClipboardWrite: true }, null, 2));
} finally {
  for (const action of ["learn_selected", "reprocess", "cancel", "transcription"]) {
    const shortcut = originalShortcuts[action].shortcut;
    await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action, shortcut })})`).catch(() => undefined);
  }
  await evaluate(`window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { transcription_to_clipboard: ${Boolean(originalSettings.transcription_to_clipboard)} } })`).catch(() => undefined);
  const encoded = Buffer.from(originalClipboard, "utf8").toString("base64");
  powershell(`Set-Clipboard -Value ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${encoded}')))`);
  socket.close();
}
