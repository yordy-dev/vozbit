import { useEffect, useEffectEvent, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { availableMonitors, getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { cancelDictation, getLastDictation, getModelStatus, getSettings, getShortcutStatus, inDesktop, listInputDevices, listenDictationError, listenMicLevel, listenShortcutState, listenShortcutToggle, patchSettings, reprocessDictation, saveHistory, startDictation, stopDictation, testInputDevice, updateShortcut } from "./api";
import { DictionaryPanel } from "./DictionaryPanel";
import { HistoryPanel } from "./HistoryPanel";
import { Icon, VoiceTextLogo } from "./Icons";
import { SettingsPanel } from "./SettingsPanel";
import Help from "./Help";
import { Tutorial } from "./Tutorial";
import type { AppSettings, AudioDevice, DictationResult, MicLevel, ModelStatus, ShortcutStatus } from "./types";
import { version as appVersion } from "../package.json";
import "./dictation.css";

type View = "home" | "history" | "dictionary" | "settings" | "about" | "help";
type DictationState = "idle" | "starting" | "recording" | "transcribing";

const DEFAULT_SETTINGS: AppSettings = {
  tutorial_version: 0,
  learn_corrections: false,
  language: "es",
  transcription_engine: "whisper",
  preferred_device_id: "default",
  auto_insert: true,
  auto_save: true,
  append_trailing_space: true,
  text_correction: true,
  format_numbers: true,
  math_symbols: true,
  push_to_talk: false,
  overlay_position: "center",
  overlay_opacity: 100,
  start_hidden: false,
  launch_at_startup: false,
  history_limit: 20,
};

function errorText(error: unknown): string { return error instanceof Error ? error.message : String(error); }

export default function App() {
  const [view, setView] = useState<View>("home");
  const [showTutorial, setShowTutorial] = useState(false);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [model, setModel] = useState<ModelStatus | null>(null);
  const [shortcut, setShortcut] = useState<ShortcutStatus | null>(null);
  const [settingsError, setSettingsError] = useState("");
  const [dictationState, setDictationState] = useState<DictationState>("idle");
  const [dictationStartedAt, setDictationStartedAt] = useState<number | null>(null);
  const [dictationElapsed, setDictationElapsed] = useState(0);
  const [dictationMaxSeconds, setDictationMaxSeconds] = useState(120);
  const [dictationResult, setDictationResult] = useState<DictationResult | null>(null);
  const [dictationError, setDictationError] = useState("");
  const [level, setLevel] = useState<MicLevel>({ rms: 0, peak: 0 });
  const [historyRevision, setHistoryRevision] = useState(0);
  const [savedHistoryId, setSavedHistoryId] = useState<number | null>(null);
  const [savingHistory, setSavingHistory] = useState(false);
  const [copyStatus, setCopyStatus] = useState("");
  const [viewBeforeSettings, setViewBeforeSettings] = useState<View>("home");
  const resizeRevision = useRef(0);
  const resizeQueue = useRef<Promise<void>>(Promise.resolve());
  const compactPosition = useRef<PhysicalPosition | null>(null);
  const wasExpanded = useRef(false);
  const dictationStateRef = useRef<DictationState>("idle");
  const stopOnReadyRef = useRef(false);
  const cancelOnReadyRef = useRef(false);
  dictationStateRef.current = dictationState;

  const modelsReady = settings.transcription_engine === "parakeet_ultra"
    ? Boolean(model?.parakeet_available && model.parakeet_sha256_ok && model.parakeet_engine_available && model.parakeet_vad_sha256_ok)
    : Boolean(model?.available && model.sha256_ok && model.engine_available && model.vad_sha256_ok);
  const whisperReady = Boolean(model?.available && model.sha256_ok && model.engine_available && model.vad_sha256_ok);
  const parakeetReady = Boolean(model?.parakeet_available && model.parakeet_sha256_ok && model.parakeet_engine_available && model.parakeet_vad_sha256_ok);

  useEffect(() => {
    if (!inDesktop) return;
    const revision = ++resizeRevision.current;
    const expanded = view === "settings" || view === "help" || view === "about";
    resizeQueue.current = resizeQueue.current.catch(() => undefined).then(async () => {
      if (revision !== resizeRevision.current) return;
      const currentWindow = getCurrentWindow();
      const origin = await currentWindow.outerPosition();
      if (expanded && !wasExpanded.current) compactPosition.current = origin;
      const desiredPosition = !expanded && wasExpanded.current && compactPosition.current ? compactPosition.current : origin;
      const width = expanded ? Math.min(960, Math.max(480, window.screen.availWidth - 40)) : 360;
      let height = expanded ? Math.min(700, Math.max(548, window.screen.availHeight - 55)) : 608;
      if (!expanded) {
        const [inner, outer, scale] = await Promise.all([
          currentWindow.innerSize(), currentWindow.outerSize(), currentWindow.scaleFactor(),
        ]);
        if (revision !== resizeRevision.current) return;
        // setSize uses the inner size; account for the native title bar and borders.
        height = Math.round(640 - (outer.height - inner.height) / scale);
      }
      await currentWindow.setSize(new LogicalSize(width, height));
      if (revision !== resizeRevision.current) return;
      const [outer, monitors] = await Promise.all([currentWindow.outerSize(), availableMonitors()]);
      const monitor = monitors.find(({ workArea }) =>
        desiredPosition.x >= workArea.position.x && desiredPosition.y >= workArea.position.y &&
        desiredPosition.x < workArea.position.x + workArea.size.width &&
        desiredPosition.y < workArea.position.y + workArea.size.height
      ) ?? monitors[0];
      if (monitor) {
        const { position, size } = monitor.workArea;
        const x = Math.max(position.x, Math.min(desiredPosition.x, position.x + size.width - outer.width));
        const y = Math.max(position.y, Math.min(desiredPosition.y, position.y + size.height - outer.height));
        if (x !== origin.x || y !== origin.y) await currentWindow.setPosition(new PhysicalPosition(x, y));
      }
      if (!expanded) compactPosition.current = null;
      wasExpanded.current = expanded;
    }).catch((reason: unknown) => setSettingsError(`No se pudo ajustar la ventana: ${errorText(reason)}`));
  }, [view]);

  useEffect(() => {
    if (!inDesktop) return;
    void getSettings().then((stored) => {
      setSettings({ ...stored, preferred_device_id: stored.preferred_device_id || "default" });
      if (stored.tutorial_version < 1) setShowTutorial(true);
      void listInputDevices().then(async (found) => {
        setDevices(found);
        const selected = stored.preferred_device_id && stored.preferred_device_id !== "default" && found.some((item) => item.id === stored.preferred_device_id) ? stored.preferred_device_id : "default";
        if (selected !== stored.preferred_device_id) {
          setSettings((current) => ({ ...current, preferred_device_id: selected }));
          await patchSettings({ preferred_device_id: selected });
        }
      }).catch((reason: unknown) => setSettingsError(`No se pudieron enumerar los micrófonos. Puedes intentar con el predeterminado de Windows: ${errorText(reason)}`));
    }).catch((reason: unknown) => setSettingsError(`No se pudo preparar la configuración: ${errorText(reason)}`));
    void getModelStatus().then(setModel).catch((reason: unknown) => setSettingsError(`No se pudo verificar el modelo: ${errorText(reason)}`));
    void getShortcutStatus().then(setShortcut).catch((reason: unknown) => setDictationError(`No se pudo verificar el atajo: ${errorText(reason)}`));
  }, []);

  useEffect(() => {
    if (!inDesktop) return;
    let active = true;
    let cleanup: (() => void) | undefined;
    void listen<string>("ui://navigate", (event) => {
      if (!active) return;
      const destination = event.payload;
      if (destination === "tutorial") { setView("home"); setShowTutorial(true); }
      else if (destination === "last") { setView("home"); void recoverLastResult(); }
      else if (["home", "dictionary", "settings", "help"].includes(destination)) {
        setShowTutorial(false);
        setView(destination as View);
      }
    }).then((unlisten) => { if (active) cleanup = unlisten; else unlisten(); });
    return () => { active = false; cleanup?.(); };
  }, []);

  useEffect(() => {
    if (!inDesktop) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void listenMicLevel((next) => { if (active) setLevel(next); }).then((fn) => { if (active) unlisten = fn; else fn(); }).catch(() => undefined);
    return () => { active = false; unlisten?.(); };
  }, []);

  useEffect(() => {
    if (!inDesktop) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void listenDictationError((message) => {
      if (!active) return;
      setDictationError(message);
      if(dictationStateRef.current==='recording') {
        dictationStateRef.current='transcribing';
        void cancelDictation(true).catch(()=>{}).finally(()=>{
          if(active){dictationStateRef.current='idle';setDictationState('idle');setDictationStartedAt(null);setLevel({rms:0,peak:0});}
        });
      }
    }).then((fn) => { if (active) unlisten = fn; else fn(); });
    return () => { active = false; unlisten?.(); };
  }, []);

  useEffect(() => {
    if (dictationState !== "recording" || dictationStartedAt === null) return;
    const timer = window.setInterval(() => setDictationElapsed((Date.now() - dictationStartedAt) / 1000), 100);
    const timeout = window.setTimeout(() => void finishDictation(), dictationMaxSeconds * 1000);
    return () => { window.clearInterval(timer); window.clearTimeout(timeout); };
  }, [dictationState, dictationStartedAt, dictationMaxSeconds]);

  const handleShortcutToggle = useEffectEvent(() => {
    if (showTutorial || document.activeElement?.getAttribute("aria-label") === "Atajo de transcripción") return;
    setView("home");
    if (dictationStateRef.current === "idle") void beginDictation(true);
    else if (dictationStateRef.current === "recording") void finishDictation();
  });

  const handleShortcutState = useEffectEvent((state: "pressed" | "released") => {
    if (showTutorial || document.activeElement?.getAttribute("aria-label") === "Atajo de transcripción") return;
    if (!settings.push_to_talk) {
      if (state === "pressed") {
        setView("home");
        if (dictationStateRef.current === "idle") void beginDictation(true);
        else if (dictationStateRef.current === "recording") void finishDictation();
      }
      return;
    }
    setView("home");
    if (state === "pressed" && dictationStateRef.current === "idle") {
      stopOnReadyRef.current = false;
      void beginDictation(true);
    }
    if (state === "released" && dictationStateRef.current === "starting") stopOnReadyRef.current = true;
    if (state === "released" && dictationStateRef.current === "recording") void finishDictation();
  });

  const handleShortcutCancel = useEffectEvent(() => {
    if (dictationStateRef.current === "recording") void abortDictation();
    else if (dictationStateRef.current === "transcribing") void invoke("transcription_cancel");
    else if (dictationStateRef.current === "starting") cancelOnReadyRef.current = true;
  });

  const handleShortcutReprocess = useEffectEvent(() => {
    if (dictationStateRef.current === "idle") void reprocessLast().catch(() => undefined);
  });

  useEffect(() => {
    if (!inDesktop) return;
    let active = true;
    const unsubscribers: Array<() => void> = [];
    let shortcutDown = false;
    void listen("shortcut://cancel", () => { if (active) handleShortcutCancel(); }).then(fn => { if (active) unsubscribers.push(fn); else fn(); });
    void listen("shortcut://reprocess", () => { if (active) handleShortcutReprocess(); }).then(fn => { if (active) unsubscribers.push(fn); else fn(); });
    void listenShortcutToggle(() => { if (active) handleShortcutToggle(); }).then(fn => { if (active) unsubscribers.push(fn); else fn(); });
    void listenShortcutState((state) => {
      if (!active) return;
      if (state === "pressed" && shortcutDown) return;
      shortcutDown = state === "pressed";
      handleShortcutState(state);
    }).then(fn => { if (active) unsubscribers.push(fn); else fn(); }).catch((reason: unknown) => setDictationError(`No se pudo escuchar el atajo: ${errorText(reason)}`));
    return () => { active = false; unsubscribers.forEach((unlisten) => unlisten()); };
  }, []);

  async function patchAppSettings(patch: Partial<AppSettings>) {
    setSettingsError("");
    try {
      await patchSettings(patch);
      setSettings((current) => ({ ...current, ...patch }));
      if (patch.history_limit !== undefined) setHistoryRevision((current) => current + 1);
    } catch (reason) {
      const message = errorText(reason);
      setSettingsError(message);
      throw reason;
    }
  }

  async function finishTutorial() {
    setShowTutorial(false);
    setView("home");
    if (settings.tutorial_version < 1) {
      try { await patchAppSettings({ tutorial_version: 1 }); }
      catch { setSettingsError("No se pudo guardar que completaste el tutorial; volverá a aparecer al iniciar."); }
    }
  }

  async function changeShortcut(next: string) {
    setSettingsError("");
    try { setShortcut(await updateShortcut(next)); }
    catch (reason) { setSettingsError(errorText(reason)); throw reason; }
  }

  async function beginDictation(fromShortcut = false) {
    if (!settings.preferred_device_id || dictationStateRef.current !== "idle" || !modelsReady) return;
    dictationStateRef.current = "starting";
    cancelOnReadyRef.current=false;
    setDictationState("starting"); setDictationError(""); setDictationResult(null); setSavedHistoryId(null); setCopyStatus(""); setDictationElapsed(0);
    try {
      const session = await startDictation(settings.preferred_device_id, fromShortcut, settings.language);
      setDictationMaxSeconds(session.max_seconds);
      setDictationStartedAt(Date.now());
      dictationStateRef.current = "recording";
      setDictationState("recording");
      if(cancelOnReadyRef.current){cancelOnReadyRef.current=false;void abortDictation();return;}
      if (stopOnReadyRef.current) {
        stopOnReadyRef.current = false;
        void finishDictation();
      }
    } catch (reason) { setDictationError(errorText(reason)); dictationStateRef.current = "idle"; setDictationState("idle"); }
  }

  async function finishDictation() {
    if (dictationStateRef.current !== "recording") return;
    dictationStateRef.current = "transcribing";
    setDictationState("transcribing"); setDictationError("");
    try {
      const completed = await stopDictation(settings.language);
      setDictationResult(completed);
      if (settings.auto_save) {
        try {
          const id = await saveHistory(completed);
          setSavedHistoryId(id);
          setHistoryRevision((current) => current + 1);
          setCopyStatus("Guardado automáticamente en Inicio.");
        } catch (reason) {
          setCopyStatus(`El texto está disponible, pero no se guardó en el historial: ${errorText(reason)}`);
        }
      }
    } catch (reason) {
      const message = errorText(reason);
      if (message === "Dictado cancelado" || message === "Transcripción cancelada") setCopyStatus("Dictado cancelado; no se insertó texto.");
      else setDictationError(message);
    }
    finally { setDictationStartedAt(null); dictationStateRef.current = "idle"; setDictationState("idle"); setLevel({ rms: 0, peak: 0 }); }
  }

  async function abortDictation() {
    if (dictationStateRef.current !== "recording") return;
    dictationStateRef.current = "transcribing";
    setDictationState("transcribing");
    try { await cancelDictation(); setCopyStatus("Grabación cancelada; no se guardó audio ni texto."); }
    catch (reason) { setDictationError(errorText(reason)); }
    finally { setDictationStartedAt(null); dictationStateRef.current = "idle"; setDictationState("idle"); setLevel({ rms: 0, peak: 0 }); }
  }

  async function reprocessLast() {
    if (dictationStateRef.current !== "idle" || !modelsReady) return;
    dictationStateRef.current = "transcribing";
    setDictationState("transcribing"); setDictationError(""); setView("home"); setCopyStatus(""); setSavedHistoryId(null);
    try {
      const completed = await reprocessDictation(settings.language);
      setDictationResult(completed);
      if (settings.auto_save) {
        try {
          const id = await saveHistory(completed);
          setSavedHistoryId(id);
          setHistoryRevision((current) => current + 1);
          setCopyStatus("Reprocesado y guardado automáticamente en Inicio.");
        } catch (reason) {
          setCopyStatus(`Reprocesado, pero no guardado en el historial: ${errorText(reason)}`);
        }
      }
    } catch (reason) {
      setDictationError(errorText(reason));
      throw reason;
    } finally {
      dictationStateRef.current = "idle";
      setDictationState("idle");
    }
  }

  async function copyResult() {
    if (!dictationResult) return;
    try { await navigator.clipboard.writeText(dictationResult.transcript.text); setCopyStatus("Texto copiado al portapapeles."); }
    catch (reason) { setCopyStatus(`No se pudo copiar: ${errorText(reason)}`); }
  }

  async function recoverLastResult() {
    setDictationError("");
    try {
      const last = await getLastDictation();
      if (!last) { setDictationError("No hay un dictado recuperable en esta sesión."); return; }
      setDictationResult(last);
      setSavedHistoryId(null);
      setCopyStatus("Recuperado de la memoria de esta sesión. Si ya está en el historial, evita guardarlo de nuevo.");
    } catch (reason) { setDictationError(`No se pudo recuperar el último texto: ${errorText(reason)}`); }
  }

  async function storeResult() {
    if (!dictationResult || savingHistory || savedHistoryId !== null) return;
    setSavingHistory(true);
    try { const id = await saveHistory(dictationResult); setSavedHistoryId(id); setHistoryRevision((current) => current + 1); setCopyStatus("Guardado en Inicio."); }
    catch (reason) { setCopyStatus(`No se pudo guardar: ${errorText(reason)}`); }
    finally { setSavingHistory(false); }
  }

  const meter = Math.min(100, Math.round(Math.sqrt(level.rms) * 100));
  const expanded = view === "settings" || view === "help" || view === "about";
  const statusText = dictationState === "recording" ? "Te escucho…" : dictationState === "transcribing" ? "Procesando en este equipo…" : dictationState === "starting" ? "Preparando el micrófono…" : !model ? "Comprobando modelos locales…" : !modelsReady ? "Revisa el motor local" : "Listo para dictar";
  const statusDescription = dictationState === "recording" ? `${dictationElapsed.toFixed(1)} de ${dictationMaxSeconds} segundos` : dictationState === "transcribing" ? "El indicador desaparecerá cuando termine." : !modelsReady && model ? "Abre Configuración para comprobar los archivos del motor." : settings.auto_insert ? "Coloca el cursor en un campo de texto y usa tu atajo." : "El texto aparecerá aquí para revisarlo y copiarlo.";
  function openSettings() { setViewBeforeSettings(view); setView("settings"); }
  function leaveExpanded() { setView(view === "settings" ? viewBeforeSettings : "settings"); }
  function updateFromHome(patch: Partial<AppSettings>) { void patchAppSettings(patch).catch(() => undefined); }

  return <>
    <div className={`vt-app ${expanded ? "vt-app-expanded" : "vt-app-compact"}`}>
      <header className="vt-topbar">
        <VoiceTextLogo />
        {expanded ? <button className="vt-top-action" type="button" onClick={leaveExpanded} aria-label="Volver">← {view === "settings" ? "Volver" : "Configuración"}</button> : <button className="vt-top-icon" type="button" onClick={openSettings} aria-label="Abrir Configuración" title="Configuración"><Icon name="settings" size={19}/></button>}
      </header>

      {expanded ? <main className="vt-expanded-main">
        {view === "settings" ? <SettingsPanel devices={devices} settings={settings} model={model} shortcut={shortcut} lastResult={dictationResult} error={settingsError} onPatch={patchAppSettings} onShortcutChange={changeShortcut} onTestMicrophone={() => testInputDevice(settings.preferred_device_id, false)} onRefreshModel={async () => { setModel(await getModelStatus()); setSettingsError(""); }} onRefreshDevices={async () => { setDevices(await listInputDevices()); setSettingsError(""); }} onReprocess={reprocessLast} onHelp={() => setView("help")} onAbout={() => setView("about")} onRepeatTutorial={() => { setView("home"); setShowTutorial(true); }}/>
        : view === "help" ? <div className="vt-expanded-page"><Help onRepeatTutorial={() => { setView("home"); setShowTutorial(true); }}/></div>
        : <div className="vt-expanded-page"><InfoPage title="Acerca de Vozbit" text="Aplicación independiente de dictado para Windows. Usa motores locales de voz; no requiere cuenta ni suscripción."/></div>}
      </main> : <>
        <main className="vt-compact-main">
          {view === "dictionary" ? <DictionaryPanel learningEnabled={settings.learn_corrections} onToggleLearning={(enabled) => patchAppSettings({ learn_corrections: enabled })} /> : view === "history" ? <><header className="vt-screen-header"><h1>Historial</h1><p>Transcripciones guardadas en este equipo.</p></header><HistoryPanel revision={historyRevision}/></> : <>
            {!inDesktop && <div className="notice">Abre la aplicación de escritorio para usar el micrófono y el motor local.</div>}
            {settingsError && <div className="error" role="alert">{settingsError}</div>}
            {shortcut && !shortcut.registered && <div className="error" role="alert">El atajo no está disponible. Elige otra combinación en Configuración.</div>}
            <section className={`surface vt-status-card ${dictationState}`} aria-live="polite">
              <div className="vt-status-heading"><span className={`vt-status-dot ${dictationState === "idle" && !modelsReady ? "warning" : dictationState}`}/><strong>{statusText}</strong></div>
              <p>{statusDescription}</p>
              <div className="vt-shortcut" data-tour="dictation"><span>Tu atajo</span><kbd>{shortcut?.shortcut ?? "Preparando atajo…"}</kbd></div>
              {dictationState === "recording" && <div className="vt-recording-meter" role="meter" aria-label="Nivel del micrófono" aria-valuemin={0} aria-valuemax={100} aria-valuenow={meter}><span style={{width:`${meter}%`}}/></div>}
            </section>
            <div className="surface vt-source-row">
              <label>Micrófono<select aria-label="Micrófono desde Inicio" value={settings.preferred_device_id} disabled={!inDesktop || dictationState !== "idle"} onChange={event => updateFromHome({preferred_device_id:event.target.value})}><option value="default">Automático — Windows</option>{devices.map(device => <option key={device.id} value={device.id}>{device.name}</option>)}</select></label>
              <label>Motor local<select aria-label="Motor local desde Inicio" value={settings.transcription_engine} disabled={!inDesktop || !model || dictationState !== "idle"} onChange={event => updateFromHome({transcription_engine:event.target.value as AppSettings["transcription_engine"]})}><option value="parakeet_ultra" disabled={!parakeetReady && settings.transcription_engine !== "parakeet_ultra"}>Parakeet Ultra{!parakeetReady ? " · revisar" : ""}</option><option value="whisper" disabled={!whisperReady && settings.transcription_engine !== "whisper"}>Whisper base{!whisperReady ? " · revisar" : ""}</option></select></label>
            </div>
            <section className="surface vt-last-result" aria-label="Último dictado">
              <div className="vt-result-heading"><strong>Último dictado</strong>{dictationResult && <small>{dictationResult.captured_seconds.toFixed(1)} s de audio</small>}</div>
              {dictationResult ? <>
                <p className="vt-result-text">{dictationResult.transcript.text}</p>
                {dictationResult.transcript.text.length > 160 && <details className="vt-result-full"><summary>Ver texto completo</summary><p>{dictationResult.transcript.text}</p></details>}
                <div className="vt-result-footer"><span>{dictationResult.delivery.sent ? "Envío aceptado por Windows · verifica el campo" : dictationResult.delivery.reason ? "No se insertó · disponible para copiar" : "Entrega manual · listo para copiar"}</span><button className="vt-copy-button" type="button" onClick={() => void copyResult()}><Icon name="copy" size={15}/>Copiar</button></div>
                {dictationResult.delivery.reason && <p className="vt-result-warning" role="alert">{dictationResult.delivery.reason}</p>}
                {dictationResult.dropped_chunks > 0 && <p className="vt-result-warning" role="alert">Se perdieron {dictationResult.dropped_chunks} fragmentos del micrófono. Revisa el texto.</p>}
                {dictationResult.dictionary_warning && <p className="vt-result-warning" role="alert">{dictationResult.dictionary_warning}</p>}
                <div className="vt-result-secondary"><button type="button" disabled={savingHistory || savedHistoryId !== null} onClick={() => void storeResult()}>{savedHistoryId !== null ? "Guardado en Historial" : savingHistory ? "Guardando…" : "Guardar en Historial"}</button><span>{dictationResult.dictionary_changes ? `${dictationResult.dictionary_changes} correcciones aplicadas` : ""}</span></div>
                {copyStatus && <p className="vt-copy-status" role="status">{copyStatus}</p>}
              </> : <p className="vt-empty-result">Aún no hay transcripciones. Tu próximo dictado aparecerá aquí.</p>}
            </section>
            {dictationError && <div className="error" role="alert">{dictationError}</div>}
            <div className="vt-home-tools">{dictationState === "idle" ? <><button type="button" disabled={!inDesktop || !modelsReady} onClick={() => void beginDictation(false)}>Probar dictado aquí</button>{!dictationResult && inDesktop && <button type="button" onClick={() => void recoverLastResult()}>Recuperar último texto</button>}</> : dictationState === "recording" ? <><button type="button" onClick={() => void finishDictation()}>Detener prueba</button><button type="button" onClick={() => void abortDictation()}>Cancelar</button></> : <span>Espera a que termine el procesamiento…</span>}</div>
          </>}
        </main>
        <nav className="vt-bottom-nav" aria-label="Navegación principal">
          <button className={view === "home" ? "active" : ""} type="button" aria-current={view === "home" ? "page" : undefined} onClick={() => setView("home")}><Icon name="home" size={18}/>Inicio</button>
          <button className={view === "history" ? "active" : ""} type="button" aria-current={view === "history" ? "page" : undefined} onClick={() => setView("history")}><Icon name="history" size={18}/>Historial</button>
          <button className={view === "dictionary" ? "active" : ""} type="button" aria-current={view === "dictionary" ? "page" : undefined} onClick={() => setView("dictionary")}><Icon name="book" size={18}/>Diccionario</button>
        </nav>
      </>}
    </div>
    {showTutorial && <Tutorial navigate={setView} finish={() => void finishTutorial()} />}
  </>;
}

function InfoPage({ title, text }: { title: string; text: string }) {
  return <><header className="page-header"><h1>{title}</h1><p>{text}</p><div className="page-rule"><span /></div></header><section className="surface info-surface"><VoiceTextLogo/><p>{text}</p><ul><li>Transcripción local con Parakeet Ultra o Whisper base.</li><li>Detección de voz con Silero VAD.</li><li>Historial y diccionario almacenados en tu equipo.</li><li>Audio temporal eliminado al salir.</li></ul><p>Versión {appVersion}</p></section></>;
}
