import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const port = process.argv[2] ?? "9472";
const diagnosticsDirectory = process.argv[3];
if (!diagnosticsDirectory) throw Error("Indica la carpeta aislada de diagnósticos");
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
function createDiagnosticWav(path, seconds = 30) {
  const sampleRate = 16_000;
  const frames = sampleRate * seconds;
  const dataBytes = frames * 2;
  const wav = Buffer.alloc(44 + dataBytes);
  wav.write("RIFF", 0); wav.writeUInt32LE(36 + dataBytes, 4); wav.write("WAVE", 8);
  wav.write("fmt ", 12); wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(sampleRate, 24); wav.writeUInt32LE(sampleRate * 2, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
  wav.write("data", 36); wav.writeUInt32LE(dataBytes, 40);
  for (let frame = 0; frame < frames; frame += 1) wav.writeInt16LE(Math.round(Math.sin(frame * Math.PI * 2 * 220 / sampleRate) * 6500), 44 + frame * 2);
  writeFileSync(path, wav);
}
function sendCancelHotkey() {
  powershell(`Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public static class VKeys { [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, System.UIntPtr extra); }'; [VKeys]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [VKeys]::keybd_event(0x12,0,0,[UIntPtr]::Zero); [VKeys]::keybd_event(0x10,0,0,[UIntPtr]::Zero); [VKeys]::keybd_event(0x75,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 60; [VKeys]::keybd_event(0x75,0,2,[UIntPtr]::Zero); [VKeys]::keybd_event(0x10,0,2,[UIntPtr]::Zero); [VKeys]::keybd_event(0x12,0,2,[UIntPtr]::Zero); [VKeys]::keybd_event(0x11,0,2,[UIntPtr]::Zero)`);
}
const originalClipboard = powershell("Get-Clipboard -Raw -ErrorAction SilentlyContinue");
const originalSettings = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
const originalShortcuts = await evaluate("window.__TAURI_INTERNALS__.invoke('shortcuts_status')");
const testShortcuts = {
  transcription: "Ctrl+Alt+Shift+F5",
  cancel: "Ctrl+Alt+Shift+F6",
  reprocess: "Ctrl+Alt+Shift+F7",
  learn_selected: "Ctrl+Alt+Shift+F8",
};
mkdirSync(diagnosticsDirectory, { recursive: true });
const diagnosticWav = join(diagnosticsDirectory, "mic-test-cancel.wav");
createDiagnosticWav(diagnosticWav);

try {
  if (await evaluate("!!document.querySelector('.tour-close')")) {
    await evaluate("document.querySelector('.tour-close').click()");
    await waitUntil("!!document.querySelector('button[aria-label=\"Abrir Configuración\"]')", "No apareció Inicio después del tutorial");
  }
  if (!(await evaluate("!!document.querySelector('.vt-settings-layout')"))) {
    const opened = await evaluate("(() => { const button = document.querySelector('button[aria-label=\"Abrir Configuración\"]'); if (!button) return false; button.click(); return true; })()");
    if (!opened) throw Error(`No se encontró el botón para abrir Configuración. Estado: ${await evaluate("JSON.stringify({href: location.href, html: document.documentElement.outerHTML.slice(0, 800)})")}`);
    await waitUntil("!!document.querySelector('.vt-settings-layout')", "No se abrió Configuración");
  }
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'General')?.click()");
  await wait(150);
  const generalFields = await evaluate("document.querySelectorAll('.vt-settings-page:not([hidden]) .shortcut-editor input').length");
  assert(generalFields === 2, "General no muestra los atajos editables de transcripción y cancelación");
  const captured = await evaluate(`(() => { const input = document.querySelector('input[aria-label="Atajo para cancelar dictado"]'); if (!input) return false; input.focus(); input.dispatchEvent(new KeyboardEvent('keydown', { key: 'F6', code: 'F6', ctrlKey: true, altKey: true, shiftKey: true, bubbles: true, cancelable: true })); return true; })()`);
  assert(captured, "No se pudo capturar el atajo de cancelación en la interfaz");
  await wait(100);
  assert(await evaluate(`document.querySelector('input[aria-label="Atajo para cancelar dictado"]')?.value === ${JSON.stringify(testShortcuts.cancel)}`), "La interfaz no mostró la nueva combinación de cancelación");
  await evaluate("document.querySelector('input[aria-label=\"Atajo para cancelar dictado\"]')?.closest('.shortcut-editor')?.querySelector('button')?.click()");
  await waitUntil(`window.__TAURI_INTERNALS__.invoke('shortcuts_status').then(status => status.cancel.shortcut === ${JSON.stringify(testShortcuts.cancel)} && status.cancel.registered)`, "La interfaz no guardó el atajo de cancelación");
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Texto y salida')?.click()");
  await wait(150);
  assert(await evaluate("document.body.innerText.includes('Transcripción a portapapeles')"), "Falta el interruptor Transcripción a portapapeles");
  assert(await evaluate("document.querySelector('.vt-settings-page:not([hidden]) .shortcut-editor input[aria-label=\"Atajo para agregar palabra seleccionada\"]') !== null"), "El atajo del diccionario no es editable");
  await evaluate("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Datos y app')?.click()");
  await wait(150);
  assert(await evaluate("document.querySelector('.vt-settings-page:not([hidden]) .shortcut-editor input[aria-label=\"Atajo para reprocesar último audio\"]') !== null"), "El atajo de reprocesado no es editable");

  for (const [action, shortcut] of Object.entries(testShortcuts).filter(([action]) => action !== "cancel")) {
    const status = await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action, shortcut })})`);
    assert(status[action].shortcut === shortcut && status[action].registered, `No se registró ${action}`);
  }
  const conflict = await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action: "cancel", shortcut: testShortcuts.transcription })}).then(() => null, error => String(error))`);
  assert(conflict?.includes("ya se usa"), "No se rechazó una combinación duplicada");

  await evaluate(`(() => { window.__cancelSmoke = 'pending'; window.__TAURI_INTERNALS__.invoke('transcription_transcribe_diagnostic', { path: ${JSON.stringify(diagnosticWav)}, language: 'es' }).then(() => { window.__cancelSmoke = 'completed'; }, error => { window.__cancelSmoke = String(error); }); return true; })()`);
  await wait(350);
  sendCancelHotkey();
  await waitUntil("window.__cancelSmoke !== 'pending'", "El atajo no detuvo el procesamiento", 80);
  const cancellation = await evaluate("window.__cancelSmoke");
  assert(cancellation.toLowerCase().includes("cancel"), `El procesamiento no terminó como cancelado: ${cancellation}`);

  await evaluate("window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { transcription_to_clipboard: true } })");
  const stored = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
  assert(stored.transcription_to_clipboard === true, "No se guardó el interruptor de portapapeles");
  const marker = `Vozbit portapapeles ${Date.now()}`;
  await evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:clipboard-manager|write_text', { text: ${JSON.stringify(marker)}, label: null })`);
  assert(powershell("Get-Clipboard -Raw").trimEnd() === marker, "El gestor nativo no escribió el texto esperado");

  console.log(JSON.stringify({ editableShortcuts: 4, cancelSavedFromUi: true, cancelStoppedProcessing: true, conflictRejected: true, clipboardSettingPersisted: true, nativeClipboardWrite: true }, null, 2));
} finally {
  for (const action of ["learn_selected", "reprocess", "cancel", "transcription"]) {
    const shortcut = originalShortcuts[action].shortcut;
    await evaluate(`window.__TAURI_INTERNALS__.invoke('shortcut_action_update', ${JSON.stringify({ action, shortcut })})`).catch(() => undefined);
  }
  await evaluate(`window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { transcription_to_clipboard: ${Boolean(originalSettings.transcription_to_clipboard)} } })`).catch(() => undefined);
  const encoded = Buffer.from(originalClipboard, "utf8").toString("base64");
  powershell(`Set-Clipboard -Value ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${encoded}')))`);
  rmSync(diagnosticWav, { force: true });
  socket.close();
}
