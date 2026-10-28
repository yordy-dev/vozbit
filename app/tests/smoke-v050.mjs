// Prueba de solo lectura contra la WebView nativa con depuración remota habilitada.
const port = process.argv[2] ?? "9450";
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find(page => page.url === "http://tauri.localhost/");
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
  if (response.exceptionDetails) throw Error(response.exceptionDetails.text);
  return response.result.value;
}
function assert(condition, message) { if (!condition) throw Error(message); }
async function clickButton(label) {
  const clicked = await evaluate(`(() => { const button = [...document.querySelectorAll('button')].find(item => item.textContent.trim() === ${JSON.stringify(label)}); if (!button) return false; button.click(); return true; })()`);
  assert(clicked, `No se encontró el botón ${label}`);
  await new Promise(resolve => setTimeout(resolve, 180));
}
try {
  await evaluate("document.querySelector('button[aria-label=\"Volver\"]')?.click()");
  await new Promise(resolve => setTimeout(resolve, 180));
  await evaluate("[...document.querySelectorAll('nav button')].find(item => item.textContent.trim() === 'Inicio')?.click()");
  await new Promise(resolve => setTimeout(resolve, 180));
  assert(await evaluate("document.querySelector('.vt-app .voice-logo svg') !== null"), "Falta el logo SVG");
  assert(await evaluate("document.querySelector('main select[aria-label=\"Micrófono desde Inicio\"]') !== null"), "Falta el micrófono en Inicio");
  assert(await evaluate("document.querySelector('main select[aria-label=\"Motor local desde Inicio\"]') !== null"), "Falta el motor en Inicio");
  const overlayVisible = await evaluate("window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', { label: 'overlay' })");
  assert(!overlayVisible, "El indicador aparece en reposo");
  const model = await evaluate("window.__TAURI_INTERNALS__.invoke('model_status')");
  const shortcut = await evaluate("window.__TAURI_INTERNALS__.invoke('shortcut_status')");
  await clickButton("Historial");
  assert(await evaluate("document.querySelector('main h1')?.textContent === 'Historial'"), "No se abrió Historial");
  await clickButton("Diccionario");
  assert(await evaluate("document.querySelector('[data-tour=\"dictionary-form\"]') !== null"), "No se abrió Diccionario");
  await evaluate("document.querySelector('button[aria-label=\"Abrir Configuración\"]')?.click()");
  await new Promise(resolve => setTimeout(resolve, 320));
  const categories = ["General", "Modelo local", "Texto y salida", "Apariencia", "Datos y app", "Ayuda y acerca de"];
  for (const category of categories) {
    await clickButton(category);
    assert(await evaluate(`document.querySelector('.vt-settings-page:not([hidden]) h1')?.textContent === ${JSON.stringify(category)}`), `No se abrió ${category}`);
  }
  await clickButton("Apariencia");
  assert(await evaluate("document.querySelector('input[aria-label=\"Opacidad del indicador\"]') !== null"), "Falta la opacidad");
  assert(await evaluate("document.querySelector('[role=\"group\"][aria-label=\"Posición del indicador\"]') !== null"), "Falta la posición");
  const overflow = await evaluate("document.documentElement.scrollWidth > document.documentElement.clientWidth + 2");
  assert(!overflow, "Hay desbordamiento horizontal");
  console.log(JSON.stringify({ version: "0.5.0", categories: categories.length, overlayAtRest: false, parakeetReady: Boolean(model.parakeet_available && model.parakeet_sha256_ok && model.parakeet_engine_available && model.parakeet_vad_sha256_ok), whisperReady: Boolean(model.available && model.sha256_ok && model.engine_available && model.vad_sha256_ok), shortcutRegistered: shortcut.registered, horizontalOverflow: false }, null, 2));
} finally {
  socket.close();
}
