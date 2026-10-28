// Recorre el tutorial nativo sin cambiar preferencias ni iniciar el micrófono.
import { mkdir, writeFile } from "node:fs/promises";

const port = process.argv[2] ?? "9452";
const outputDirectory = process.argv[3];
const smallViewport = process.argv[4] === "small";
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
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
function assert(value, message) { if (!value) throw Error(message); }
async function click(expression, label) {
  assert(await evaluate(`(() => { const button = ${expression}; if (!button) return false; button.click(); return true; })()`), `No se encontró ${label}`);
  await wait(220);
}
async function capture(name) {
  if (!outputDirectory) return;
  const image = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await writeFile(`${outputDirectory}/tutorial-${name}-v052.png`, Buffer.from(image.data, "base64"));
}
const geometry = `(() => {
  const card = document.querySelector('.tour-card')?.getBoundingClientRect();
  const focus = document.querySelector('.tour-focus')?.getBoundingClientRect();
  const title = document.querySelector('.tour-card h2')?.textContent;
  if (!card) return null;
  const overlap = focus ? Math.max(0, Math.min(card.right, focus.right) - Math.max(card.left, focus.left)) * Math.max(0, Math.min(card.bottom, focus.bottom) - Math.max(card.top, focus.top)) : 0;
  return { title, viewport: [innerWidth, innerHeight], card: [card.left, card.top, card.width, card.height], focus: focus ? [focus.left, focus.top, focus.width, focus.height] : null, overlap };
})()`;

await send("Page.enable");
if (smallViewport) await send("Emulation.setDeviceMetricsOverride", { width: 440, height: 500, deviceScaleFactor: 1, mobile: false });
let originalVersion;
try {
  originalVersion = (await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')")).tutorial_version;
  if (!(await evaluate("!!document.querySelector('.tour-welcome')"))) {
    if (!(await evaluate("!!document.querySelector('.vt-settings-layout')"))) {
      await click("document.querySelector('button[aria-label=\"Abrir Configuración\"]')", "Configuración");
    }
    await click("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Ayuda y acerca de')", "Ayuda y acerca de");
    await click("[...document.querySelectorAll('button')].find(item => item.textContent.trim() === 'Repetir tutorial')", "Repetir tutorial");
  }
  assert(await evaluate("!!document.querySelector('.tour-welcome')"), "Falta la bienvenida del tutorial");
  const welcomeWidth = await evaluate("document.querySelector('.tour-welcome').getBoundingClientRect().width");
  assert(welcomeWidth <= 352, `La bienvenida es demasiado ancha: ${welcomeWidth}`);
  await capture("bienvenida");
  await click("document.querySelector('.tour-welcome .tour-primary')", "Ver recorrido");
  const results = [];
  for (let step = 0; step < 5; step++) {
    await wait(step === 1 || step === 3 ? 650 : 300);
    const result = await evaluate(geometry);
    assert(result, `Falta la tarjeta del paso ${step + 1}`);
    const [left, top, width, height] = result.card;
    const [viewportWidth, viewportHeight] = result.viewport;
    assert(width <= 320 && left >= 10 && top >= 10 && left + width <= viewportWidth - 10 && top + height <= viewportHeight - 10, `Tarjeta fuera de pantalla en ${result.title}: ${JSON.stringify(result)}`);
    if (step < 4) {
      assert(result.focus, `Falta el control resaltado en ${result.title}`);
      assert(result.overlap < 1, `La tarjeta tapa el control en ${result.title}: ${JSON.stringify(result)}`);
    }
    results.push({ title: result.title, card: result.card.map(Math.round), overlap: result.overlap });
    if (!smallViewport) await capture(`paso-${step + 1}`);
    await click("document.querySelector('.tour-card .tour-primary')", step === 4 ? "Listo" : "Siguiente");
  }
  assert(!(await evaluate("!!document.querySelector('.tour-layer')")), "El tutorial no se cerró al terminar");
  console.log(JSON.stringify({ viewport: smallViewport ? "440x500" : "nativo", welcomeWidth, steps: results, completed: true }, null, 2));
} finally {
  if (originalVersion !== undefined) {
    const currentVersion = (await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')")).tutorial_version;
    if (currentVersion !== originalVersion) await evaluate(`window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { tutorial_version: ${JSON.stringify(originalVersion)} } })`);
  }
  if (smallViewport) await send("Emulation.clearDeviceMetricsOverride");
  socket.close();
}
