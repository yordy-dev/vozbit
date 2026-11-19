// Auditoría visual y funcional de solo lectura para la WebView nativa de Vozbit.
import { mkdir, writeFile } from "node:fs/promises";

const port = process.argv[2] ?? "9461";
const outputDirectory = process.argv[3];
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
function assert(value, message) { if (!value) throw Error(message); }
async function waitUntil(expression, label, attempts = 60) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (await evaluate(expression)) return;
    await wait(100);
  }
  throw Error(`Tiempo agotado esperando ${label}`);
}
async function click(expression, label) {
  const clicked = await evaluate(`(() => { const target = ${expression}; if (!target) return false; target.click(); return true; })()`);
  assert(clicked, `No se encontró ${label}`);
  await wait(260);
}
async function capture(name) {
  if (!outputDirectory) return;
  const image = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await writeFile(`${outputDirectory}/${name}.png`, Buffer.from(image.data, "base64"));
}
async function inspectScreen(expected) {
  const result = await evaluate(`(() => {
    const app = document.querySelector('.vt-app');
    const visible = [...document.querySelectorAll('button,input,select,textarea,summary,a')].filter(node => {
      const rect = node.getBoundingClientRect();
      const style = getComputedStyle(node);
      return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
    });
    const targetRect = node => node instanceof HTMLInputElement && ['checkbox','radio','range'].includes(node.type)
      ? (node.closest('label') ?? node.parentElement ?? node).getBoundingClientRect()
      : node.getBoundingClientRect();
    const undersized = visible.filter(node => targetRect(node).height < 36)
      .map(node => ({ tag: node.tagName, text: node.textContent?.trim().slice(0,40), label: node.getAttribute('aria-label'), height: Math.round(targetRect(node).height) }));
    return {
      theme: app?.getAttribute('data-theme'),
      viewport: [innerWidth, innerHeight],
      horizontalOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 2,
      controls: visible.length,
      undersized,
      heading: [...document.querySelectorAll('.vt-settings-page h1,main h1')]
        .find(node => getComputedStyle(node.closest('.vt-settings-page') ?? node).display !== 'none')?.textContent ?? null,
      bodyText: document.body.innerText,
    };
  })()`);
  assert(!result.horizontalOverflow, `${expected}: desbordamiento horizontal`);
  assert(result.undersized.length === 0, `${expected}: controles pequeños ${JSON.stringify(result.undersized)}`);
  return result;
}

await send("Page.enable");
if (outputDirectory) await mkdir(outputDirectory, { recursive: true });
const original = await evaluate("window.__TAURI_INTERNALS__.invoke('settings_get')");
const report = { themes: {}, screens: {} };
try {
  if (await evaluate("!!document.querySelector('.tour-layer')")) await click("document.querySelector('.tour-close')", "cerrar tutorial");
  if (await evaluate("!!document.querySelector('button[aria-label=\"Volver\"]')")) await click("document.querySelector('button[aria-label=\"Volver\"]')", "Volver");
  await click("[...document.querySelectorAll('.vt-bottom-nav button')].find(item => item.textContent.trim() === 'Inicio')", "Inicio");

  for (const theme of ["system", "light", "dark", "contrast"]) {
    await click("document.querySelector('button[aria-label=\"Abrir Configuración\"]')", "Configuración");
    await click("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Apariencia')", "Apariencia");
    await click(`document.querySelector('.vt-theme-option.is-${theme}')`, `tema ${theme}`);
    await waitUntil(`window.__TAURI_INTERNALS__.invoke('settings_get').then(value => value.theme === ${JSON.stringify(theme)} && !document.querySelector('.vt-theme-option.is-${theme}')?.disabled)`, `persistencia del tema ${theme}`);
    const appliedTheme = await evaluate("document.querySelector('.vt-app')?.getAttribute('data-theme')");
    assert(appliedTheme === theme, `No se aplicó ${theme}; tema visible: ${appliedTheme}`);
    assert(await evaluate(`document.querySelector('.vt-theme-option.is-${theme}')?.getAttribute('aria-pressed') === 'true'`), `No se seleccionó ${theme}`);
    const appearance = await inspectScreen(`Apariencia ${theme}`);
    await capture(`apariencia-${theme}`);
    await click("document.querySelector('button[aria-label=\"Volver\"]')", "Volver");
    const home = await inspectScreen(`Inicio ${theme}`);
    assert(home.bodyText.includes("Micrófono") && home.bodyText.includes("Motor local") && home.bodyText.includes("Último dictado"), `Inicio ${theme}: faltan controles principales`);
    await capture(`inicio-${theme}`);
    report.themes[theme] = { appearanceControls: appearance.controls, homeControls: home.controls };
  }

  await click("[...document.querySelectorAll('.vt-bottom-nav button')].find(item => item.textContent.trim() === 'Historial')", "Historial");
  report.screens.history = await inspectScreen("Historial");
  assert(report.screens.history.heading === "Historial", "No se abrió Historial");
  await capture("pantalla-historial");

  await click("[...document.querySelectorAll('.vt-bottom-nav button')].find(item => item.textContent.trim() === 'Diccionario')", "Diccionario");
  report.screens.dictionary = await inspectScreen("Diccionario");
  assert(await evaluate("!!document.querySelector('[data-tour=\"dictionary-form\"]')"), "Falta el formulario del Diccionario");
  await capture("pantalla-diccionario");

  await click("document.querySelector('button[aria-label=\"Abrir Configuración\"]')", "Configuración");
  const categories = ["General", "Modelo local", "Texto y salida", "Apariencia", "Datos y app", "Ayuda y acerca de"];
  for (const category of categories) {
    await click(`[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === ${JSON.stringify(category)})`, category);
    await waitUntil(`document.querySelector('.vt-settings-sidebar button.active')?.textContent.trim() === ${JSON.stringify(category)}`, `categoría ${category}`);
    const inspected = await inspectScreen(category);
    assert(inspected.heading === category, `Encabezado incorrecto en ${category}: ${inspected.heading}`);
    report.screens[category] = { controls: inspected.controls };
    await capture(`config-${category.toLowerCase().replaceAll(' ', '-').replaceAll('y-acerca-de','ayuda')}`);
  }

  await click("[...document.querySelectorAll('button')].find(item => item.textContent.trim() === 'Abrir ayuda')", "Abrir ayuda");
  const help = await inspectScreen("Ayuda");
  assert(help.heading === "Dictar donde escribes", "No se abrió Ayuda");
  report.screens.help = { controls: help.controls };
  await capture("pantalla-ayuda");
  await click("document.querySelector('button[aria-label=\"Volver\"]')", "Configuración");
  await click("[...document.querySelectorAll('.vt-settings-sidebar button')].find(item => item.textContent.trim() === 'Ayuda y acerca de')", "Ayuda y acerca de");
  await click("[...document.querySelectorAll('button')].find(item => item.textContent.trim() === 'Ver información')", "Ver información");
  const about = await inspectScreen("Acerca de");
  assert(about.heading === "Acerca de Vozbit", "No se abrió Acerca de");
  report.screens.about = { controls: about.controls };
  await capture("pantalla-acerca-de");

  console.log(JSON.stringify(report, null, 2));
} finally {
  await evaluate(`window.__TAURI_INTERNALS__.invoke('settings_patch', { patch: { theme: ${JSON.stringify(original.theme)} } })`).catch(() => undefined);
  socket.close();
}
