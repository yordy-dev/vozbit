// Lectura de la interfaz empaquetada mediante el puerto de depuración de WebView2.
// No modifica ajustes, diccionario, historial, audio ni otros archivos del usuario.
import { writeFile } from "node:fs/promises";
const port = process.argv[2] ?? "9448";
const screenshotPath = process.argv[3];
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find((page) => page.url === "http://tauri.localhost/");
if (!main) throw Error("No se encontró la ventana principal empaquetada");
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
  if (response.exceptionDetails) throw Error(response.exceptionDetails.text);
  return response.result.value;
}
function assert(condition, message) { if (!condition) throw Error(message); }
async function navigate(label) {
  const found = await evaluate(`(() => { const button = [...document.querySelectorAll('nav button')].find(item => item.textContent.trim() === ${JSON.stringify(label)}); if (!button) return false; button.click(); return true; })()`);
  assert(found, `No aparece ${label} en la navegación`);
  await new Promise((resolve) => setTimeout(resolve, 120));
  const heading = await evaluate("document.querySelector('.page-header h1')?.textContent");
  assert(heading?.includes(label === "Ayuda" ? "Dictar donde escribes" : label), `No se abrió ${label}`);
  const current = await evaluate("[...document.querySelectorAll('nav button[aria-current=page]')].map(item => item.textContent.trim())");
  assert(current.length === 1 && current[0] === label, `Navegación accesible incorrecta en ${label}`);
}
try {
  const version = await evaluate("document.querySelector('.side-bottom small')?.textContent");
  assert(version?.includes("0.4.8"), `Versión inesperada: ${version}`);
  const tutorial = await evaluate("Boolean(document.querySelector('.tour-layer'))");
  assert(!tutorial, "El tutorial inicial está abierto; omítelo manualmente y repite la prueba");
  const overlayVisible = await evaluate("window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', { label: 'overlay' })");
  assert(!overlayVisible, "El indicador está visible sin grabación");
  const settings = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
  assert(!("overlay_style" in settings), "Persiste el ajuste obsoleto del indicador");
  const model = await evaluate("window.__TAURI_INTERNALS__.invoke('model_status').then(s => ({available:s.available,sha256_ok:s.sha256_ok,engine_available:s.engine_available,vad_sha256_ok:s.vad_sha256_ok}))");
  assert(model.available && model.sha256_ok && model.engine_available && model.vad_sha256_ok, "Falta el modelo, el detector de voz o el motor local");
  const shortcut = await evaluate("window.__TAURI_INTERNALS__.invoke('shortcut_status').then(s => ({registered:s.registered}))");
  await navigate("Inicio");
  await navigate("Diccionario");
  const dictionary = await evaluate("Boolean(document.querySelector('[data-tour=dictionary-form]')) && Boolean(document.querySelector('.dictionary-preview textarea'))");
  assert(dictionary, "Falta el formulario o la prueba de frase del diccionario");
  await navigate("Configuración");
  const headings = await evaluate("[...document.querySelectorAll('.settings-heading')].map(item => item.textContent.trim())");
  for (const title of ["GENERAL", "TRANSCRIPCIÓN LOCAL", "SALIDA", "APLICACIÓN", "HISTORIAL", "DIAGNÓSTICO LOCAL"]) assert(headings.includes(title), `Falta ${title}`);
  const positions = await evaluate("[...document.querySelectorAll('select[aria-label=\"Posición de la ventana de grabación\"] option')].map(item => item.value)");
  assert(JSON.stringify(positions) === JSON.stringify(["top_center", "center", "bottom_center"]), "Posiciones del indicador incorrectas");
  const metrics = await evaluate("Boolean(document.querySelector('section[aria-label=\"Diagnóstico local sin contenido del dictado\"]'))");
  assert(metrics, "Falta el diagnóstico local");
  const overflow = await evaluate("document.documentElement.scrollWidth > document.documentElement.clientWidth + 2");
  assert(!overflow, "Hay desbordamiento horizontal en Configuración");
  if (screenshotPath) {
    await send("Page.enable");
    const screenshot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));
  }
  await navigate("Acerca de");
  await navigate("Ayuda");
  await navigate("Inicio");
  console.log(JSON.stringify({ version, pages: 5, sections: headings.length, overlayAtRest: false, dictionary: true, diagnostics: true, modelReady: true, shortcutRegistered: shortcut.registered, horizontalOverflow: false, learningEnabled: settings.learn_corrections }, null, 2));
} finally {
  socket.close();
}
