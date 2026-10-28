// Prueba controlada del indicador. Abre el micrófono y cancela sin transcribir ni guardar.
import { writeFile } from "node:fs/promises";
const port = process.argv[2] ?? "9448";
const screenshotPath = process.argv[3];
const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const main = pages.find((page) => page.url === "http://tauri.localhost/");
const overlay = pages.find((page) => page.url.includes("overlay=1"));
if (!main || !overlay) throw Error("Faltan ventanas de la aplicación empaquetada");
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
async function invoke(command, args = {}) {
  const expression = `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`;
  const response = await mainCdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (response.exceptionDetails) throw Error(response.exceptionDetails.text);
  return response.result.value;
}
let recording = false;
try {
  if (await invoke("plugin:window|is_visible", { label: "overlay" })) throw Error("El indicador apareció antes de iniciar");
  await invoke("dictation_start", { deviceId: null, deliveryRequested: true });
  recording = true;
  if (!(await invoke("plugin:window|is_visible", { label: "overlay" }))) throw Error("El indicador no apareció al grabar");
  if (screenshotPath) {
    await overlayCdp.send("Page.enable");
    const shot = await overlayCdp.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
    await writeFile(screenshotPath, Buffer.from(shot.data, "base64"));
  }
  await invoke("dictation_cancel");
  recording = false;
  if (await invoke("plugin:window|is_visible", { label: "overlay" })) throw Error("El indicador permaneció visible al cancelar");
  console.log(JSON.stringify({ microphoneStarted: true, overlayDuringRecording: true, overlayAfterCancel: false, audioSaved: false }, null, 2));
} finally {
  if (recording) await invoke("dictation_cancel").catch(() => undefined);
  mainCdp.socket.close();
  overlayCdp.socket.close();
}
