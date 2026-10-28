// Prueba del indicador nativo con micrófono. Restaura la entrega automática al terminar.
import { mkdir, writeFile } from "node:fs/promises";
const port = process.argv[2] ?? "9451";
const outputDirectory = process.argv[3];
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find(page => page.url === "http://tauri.localhost/");
const overlay = pages.find(page => page.url.includes("overlay=1"));
if (!main || !overlay) throw Error("No se encontraron las dos ventanas de VoiceText Local");

async function connect(page) {
  const socket = new WebSocket(page.webSocketDebuggerUrl);
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
  return {
    socket,
    send(method, params = {}) {
      const id = ++nextId;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
  };
}

const mainCdp = await connect(main);
const overlayCdp = await connect(overlay);
if (outputDirectory) {
  await mkdir(outputDirectory, { recursive: true });
  await overlayCdp.send("Page.enable");
}
async function capture(name) {
  if (!outputDirectory) return;
  const image = await overlayCdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await writeFile(`${outputDirectory}/indicador-${name}-v051.png`, Buffer.from(image.data, "base64"));
}
async function evaluate(cdp, expression) {
  const response = await cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (response.exceptionDetails) throw Error(response.exceptionDetails.text);
  return response.result.value;
}
const invoke = (command, args = {}) => evaluate(mainCdp, `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`);
const visible = () => invoke("plugin:window|is_visible", { label: "overlay" });
const phase = () => evaluate(overlayCdp, "document.querySelector('.overlay-shell')?.className ?? ''");
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
async function waitForPhase(expected) {
  for (let attempt = 0; attempt < 40; attempt++) {
    if ((await phase()).includes(expected)) return;
    await wait(50);
  }
  throw Error(`El indicador no pasó a ${expected}: ${await phase()}`);
}
function assert(value, description) { if (!value) throw Error(description); }

let originalAutoInsert;
let recording = false;
try {
  const settings = await invoke("settings_get");
  originalAutoInsert = settings.auto_insert;
  await invoke("settings_patch", { patch: { auto_insert: false } });
  assert(!(await visible()), "El indicador estaba visible en reposo");

  await invoke("dictation_start", { deviceId: null, deliveryRequested: true, language: "es" });
  recording = true;
  assert(await visible(), "El indicador no apareció al grabar");
  await waitForPhase("recording");
  await capture("grabando");
  await invoke("dictation_cancel", { showError: true });
  recording = false;
  assert(await visible(), "El error no quedó visible");
  await waitForPhase("error");
  await capture("error");
  await wait(3100);
  assert(!(await visible()), "El error no se cerró al cabo del tiempo previsto");

  await invoke("dictation_start", { deviceId: null, deliveryRequested: true, language: "es" });
  recording = true;
  await waitForPhase("recording");
  await wait(1200);
  let finished = false;
  const stop = invoke("dictation_stop", { language: "es" }).then(value => ({ ok: true, value }), error => ({ ok: false, error: String(error) })).finally(() => { finished = true; });
  recording = false;
  let sawProcessing = false;
  for (let attempt = 0; attempt < 80 && !finished; attempt++) {
    if ((await phase()).includes("processing")) {
      assert(await visible(), "El indicador se ocultó durante el procesamiento");
      sawProcessing = true;
      await capture("procesando");
      break;
    }
    await wait(50);
  }
  const outcome = await stop;
  assert(sawProcessing, "No se observó el estado ámbar durante el procesamiento");
  if (!outcome.ok) {
    await waitForPhase("error");
    await wait(3100);
  }
  assert(!(await visible()), "El indicador permaneció visible al finalizar");

  await invoke("dictation_start", { deviceId: null, deliveryRequested: true, language: "es" });
  recording = true;
  await waitForPhase("recording");
  await invoke("dictation_cancel", { showError: false });
  recording = false;
  assert(!(await visible()), "El indicador siguió visible tras cancelar");
  assert(!(await phase()).includes("error"), "La cancelación se marcó como error");
  console.log(JSON.stringify({ recording: true, processingVisible: sawProcessing, errorVisible: true, cancellationWithoutError: true, transcriptionOutcome: outcome.ok ? "completada" : "error de audio silencioso" }, null, 2));
} finally {
  if (recording) await invoke("dictation_cancel", { showError: false }).catch(() => undefined);
  if (originalAutoInsert !== undefined) await invoke("settings_patch", { patch: { auto_insert: originalAutoInsert } }).catch(() => undefined);
  mainCdp.socket.close();
  overlayCdp.socket.close();
}
