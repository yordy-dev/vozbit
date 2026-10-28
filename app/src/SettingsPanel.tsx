import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { VoiceTextMark } from "./Icons";
import type { AppSettings, AudioDevice, AudioTestResult, DictationResult, ModelStatus, ShortcutStatus } from "./types";
import "./settings.css";

type Category = "general" | "model" | "output" | "appearance" | "data" | "help";

interface Props {
  devices: AudioDevice[];
  settings: AppSettings;
  model: ModelStatus | null;
  shortcut: ShortcutStatus | null;
  lastResult: DictationResult | null;
  error: string;
  onPatch: (patch: Partial<AppSettings>) => Promise<void>;
  onShortcutChange: (shortcut: string) => Promise<void>;
  onTestMicrophone: () => Promise<AudioTestResult>;
  onRefreshModel: () => Promise<void>;
  onRefreshDevices: () => Promise<void>;
  onReprocess: () => Promise<void>;
  onHelp: () => void;
  onAbout: () => void;
  onRepeatTutorial: () => void;
}

function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return <div className="setting-row"><div><strong>{title}</strong>{hint && <small>{hint}</small>}</div>{children}</div>;
}

const categories: { id: Category; label: string }[] = [
  { id: "general", label: "General" },
  { id: "model", label: "Modelo local" },
  { id: "output", label: "Texto y salida" },
  { id: "appearance", label: "Apariencia" },
  { id: "data", label: "Datos y app" },
  { id: "help", label: "Ayuda y acerca de" },
];

export function SettingsPanel(p: Props) {
  const [category, setCategory] = useState<Category>("general");
  const [candidate, setCandidate] = useState(p.shortcut?.shortcut ?? "Ctrl+Shift+Space");
  const [message, setMessage] = useState("");
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [hasAudio, setHasAudio] = useState(false);
  const [opacityDraft, setOpacityDraft] = useState(p.settings.overlay_opacity);
  const busyRef = useRef(false);
  const contentRef = useRef<HTMLDivElement>(null);
  const whisperReady = Boolean(p.model?.available && p.model.sha256_ok && p.model.engine_available && p.model.vad_sha256_ok);
  const parakeetReady = Boolean(p.model?.parakeet_available && p.model.parakeet_sha256_ok && p.model.parakeet_engine_available && p.model.parakeet_vad_sha256_ok);

  useEffect(() => { void invoke<boolean>("dictation_status").then(setHasAudio).catch(() => undefined); }, []);
  useEffect(() => { if (p.shortcut) setCandidate(p.shortcut.shortcut); }, [p.shortcut]);
  useEffect(() => setOpacityDraft(p.settings.overlay_opacity), [p.settings.overlay_opacity]);
  useEffect(() => {
    const selectTourCategory = (event: Event) => {
      const next = (event as CustomEvent<Category>).detail;
      if (categories.some(item => item.id === next)) setCategory(next);
    };
    window.addEventListener("voicetext:settings-category", selectTourCategory);
    return () => window.removeEventListener("voicetext:settings-category", selectTourCategory);
  }, []);
  useEffect(() => { if (contentRef.current) contentRef.current.scrollTop = 0; }, [category]);

  async function action(task: () => Promise<unknown>, success = "Guardado") {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setFailed(false);
    setMessage("Procesando…");
    try {
      const result = await task();
      setMessage(typeof result === "string" ? result : success);
    } catch (error) {
      setFailed(true);
      setMessage(String(error));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }

  const patch = (value: Partial<AppSettings>) => void action(() => p.onPatch(value));
  const toggle = (key: keyof AppSettings, label: string) => <button type="button" role="switch" className="switch" aria-label={label} aria-checked={Boolean(p.settings[key])} disabled={busy} onClick={() => patch({ [key]: !p.settings[key] })}><span /></button>;
  const selectCategory = (next: Category) => { setCategory(next); setMessage(""); setFailed(false); };

  return <div className="vt-settings-layout">
    <aside className="vt-settings-sidebar" aria-label="Categorías de configuración">
      <p>CONFIGURACIÓN</p>
      <nav>{categories.map(item => <button key={item.id} type="button" className={category === item.id ? "active" : ""} aria-current={category === item.id ? "page" : undefined} onClick={() => selectCategory(item.id)}>{item.label}</button>)}</nav>
      <div className="vt-settings-tail">Sin cuenta · datos en este equipo.</div>
    </aside>

    <div className="vt-settings-content" ref={contentRef}>
      {(message || p.error) && <div className={`settings-feedback ${failed || p.error ? "is-error" : ""}`} role={failed || p.error ? "alert" : "status"}>{p.error || message}</div>}

      <section className="vt-settings-page" hidden={category !== "general"}>
        <h1>General</h1><p className="vt-settings-lead">Atajo, modo y micrófono para dictar desde cualquier aplicación.</p>
        <h2 className="vt-settings-group-label">CONTROL DEL DICTADO</h2><div className="surface settings-card">
          <Row title="Atajo de transcripción" hint="Haz clic y pulsa una combinación. Si Ctrl + Shift interfiere con tu editor, elige otra."><div className="setting-control" data-tour="shortcut"><input aria-label="Atajo de transcripción" value={candidate} disabled={busy} onChange={event => setCandidate(event.target.value)} onKeyDown={event => { if (event.key === "Tab") return; event.preventDefault(); if (["Control", "Alt", "Shift", "Meta"].includes(event.key)) { if (event.ctrlKey && event.shiftKey && !event.altKey) setCandidate("Ctrl+Shift"); return; } setCandidate([event.ctrlKey ? "Ctrl" : "", event.altKey ? "Alt" : "", event.shiftKey ? "Shift" : "", event.key === " " ? "Space" : event.key.length === 1 ? event.key.toUpperCase() : event.key].filter(Boolean).join("+")); }} /><button type="button" disabled={busy || candidate === p.shortcut?.shortcut} onClick={() => void action(() => p.onShortcutChange(candidate))}>Guardar</button><button type="button" disabled={busy} title="Restaurar Ctrl + Shift + Espacio" aria-label="Restaurar atajo" onClick={() => { setCandidate("Ctrl+Shift+Space"); void action(() => p.onShortcutChange("Ctrl+Shift+Space")); }}>↺</button></div></Row>
          <Row title="Modo de grabación" hint="El indicador no tiene botones; se controla con el teclado."><select disabled={busy} aria-label="Modo de grabación" value={p.settings.push_to_talk ? "hold" : "toggle"} onChange={event => patch({ push_to_talk: event.target.value === "hold" })}><option value="toggle">Pulsar para iniciar / detener</option><option value="hold">Mantener y soltar</option></select></Row>
          <Row title="Cancelar dictado" hint="Detiene la grabación o el procesamiento sin entregar texto."><kbd>Ctrl + Alt + Esc</kbd></Row>
        </div>
        <h2 className="vt-settings-group-label">MICRÓFONO E IDIOMA</h2><div className="surface settings-card">
          <Row title="Micrófono" hint="Automático sigue el dispositivo predeterminado de Windows."><div className="setting-control"><select disabled={busy} aria-label="Micrófono" value={p.settings.preferred_device_id} onChange={event => patch({ preferred_device_id: event.target.value })}><option value="default">Automático — Windows</option>{p.devices.map(device => <option key={device.id} value={device.id}>{device.name}</option>)}</select><button type="button" disabled={busy} aria-label="Actualizar micrófonos" onClick={() => void action(p.onRefreshDevices, "Micrófonos actualizados")}>↺</button></div></Row>
          <Row title="Comprobar micrófono" hint="Prueba de cinco segundos sin conservar audio."><button className="soft-button" type="button" disabled={busy} onClick={() => void action(async () => { const result = await p.onTestMicrophone(); return `Prueba terminada · Pico ${Math.round(result.peak * 100)} % · ${result.dropped_chunks} fragmentos perdidos`; })}>Probar entrada</button></Row>
          <Row title="Idioma"><select disabled={busy} aria-label="Idioma" value={p.settings.language} onChange={event => patch({ language: event.target.value as AppSettings["language"] })}><option value="es">Español</option><option value="en">Inglés</option><option value="auto">Automático</option></select></Row>
        </div>
      </section>

      <section className="vt-settings-page" hidden={category !== "model"}>
        <h1>Modelo local</h1><p className="vt-settings-lead">El reconocimiento se ejecuta en tu equipo, sin enviar audio a un servicio.</p>
        <h2 className="vt-settings-group-label">MOTOR DE TRANSCRIPCIÓN</h2><div className="surface settings-card">
          <Row title="Motor de voz" hint="Solo puedes cambiar a un motor cuyos archivos estén listos."><select disabled={busy || !p.model} aria-label="Motor de transcripción" value={p.settings.transcription_engine} onChange={event => patch({ transcription_engine: event.target.value as AppSettings["transcription_engine"] })}><option value="parakeet_ultra" disabled={!parakeetReady && p.settings.transcription_engine !== "parakeet_ultra"}>Parakeet Ultra{!parakeetReady ? " · revisar archivos" : ""}</option><option value="whisper" disabled={!whisperReady && p.settings.transcription_engine !== "whisper"}>Whisper base{!whisperReady ? " · revisar archivos" : ""}</option></select></Row>
          <Row title="Parakeet Ultra" hint="Carga el modelo durante la grabación; procesa el audio completo al soltar."><span className={`status-badge ${parakeetReady ? "ready" : "problem"}`}>{parakeetReady ? "Disponible" : "Revisar archivos"}</span></Row>
          <Row title="Whisper base" hint="Alternativa local que procesa fragmentos mientras hablas."><span className={`status-badge ${whisperReady ? "ready" : "problem"}`}>{whisperReady ? "Disponible" : "Revisar archivos"}</span></Row>
          <Row title="Comprobar modelos" hint="Verifica integridad y disponibilidad de los motores."><button className="soft-button" type="button" disabled={busy} onClick={() => void action(p.onRefreshModel, "Modelos comprobados")}>Comprobar</button></Row>
        </div>
      </section>

      <section className="vt-settings-page" hidden={category !== "output"}>
        <h1>Texto y salida</h1><p className="vt-settings-lead">Elige dónde llega el dictado y cómo se formatea.</p>
        <h2 className="vt-settings-group-label">ENTREGA</h2><div className="surface settings-card">
          <Row title="Entrega del texto" hint="Con el cursor en otro campo, Windows acepta el envío; comprueba el resultado en el editor."><select data-tour="delivery" disabled={busy} aria-label="Entrega del texto" value={p.settings.auto_insert ? "direct" : "manual"} onChange={event => patch({ auto_insert: event.target.value === "direct" })}><option value="direct">Escribir en el campo activo</option><option value="manual">Revisar en Inicio y copiar</option></select></Row>
          <Row title="Puntuación y mayúsculas" hint="Corrige órdenes explícitas, espacios y mayúsculas; no reescribe frases.">{toggle("text_correction", "Puntuación y mayúsculas")}</Row>
          <Row title="Números en cifras">{toggle("format_numbers", "Números en cifras")}</Row>
          <Row title="Operadores matemáticos">{toggle("math_symbols", "Operadores matemáticos")}</Row>
          <Row title="Espacio al terminar">{toggle("append_trailing_space", "Espacio al terminar")}</Row>
        </div>
        <h2 className="vt-settings-group-label">DICCIONARIO</h2><div className="surface settings-card">
          <Row title="Aprender de mis correcciones" hint="Opcional: observa durante 30 segundos el campo donde se insertó el último dictado. Guarda palabras claras automáticamente; revisa cambios ambiguos en Diccionario.">{toggle("learn_corrections", "Aprender de mis correcciones")}</Row>
          <Row title="Agregar palabra seleccionada" hint="Selecciona texto en un editor compatible."><kbd>Ctrl + Alt + L</kbd></Row>
        </div>
      </section>

      <section className="vt-settings-page" hidden={category !== "appearance"}>
        <h1>Apariencia</h1><p className="vt-settings-lead">Aspecto de la interfaz y del indicador mini.</p>
        <h2 className="vt-settings-group-label">INTERFAZ</h2><div className="surface settings-card">
          <Row title="Tema de la interfaz" hint="Esta versión utiliza el tema claro."><span className="status-badge pending">Claro</span></Row>
        </div>
        <h2 className="vt-settings-group-label">INDICADOR MINI</h2><div className="surface settings-card">
          <Row title="Vista previa" hint="Aparece solo durante grabación y procesamiento. No toma el foco ni tiene botones."><div className={`vt-mini-well ${p.settings.overlay_position}`}><div className="vt-mini-pill" style={{ background: `rgba(32,42,37,${opacityDraft / 100})` }}><VoiceTextMark size={19}/><span className="vt-mini-divider"/><span className="vt-mini-wave" aria-hidden="true">{Array.from({ length: 12 }, (_, index) => <i key={index}/>)}</span></div></div></Row>
          <Row title="Posición" hint="Arriba, centro o abajo de la pantalla."><div className="vt-segmented" role="group" aria-label="Posición del indicador">{([ ["top_center", "Arriba"], ["center", "Centro"], ["bottom_center", "Abajo"] ] as const).map(([value, label]) => <button key={value} type="button" disabled={busy} aria-pressed={p.settings.overlay_position === value} onClick={() => patch({ overlay_position: value })}>{label}</button>)}</div></Row>
          <Row title="Opacidad del fondo" hint="El ícono y las ondas conservan su contraste."><div className="opacity-control"><input type="range" min="40" max="100" step="5" value={opacityDraft} aria-label="Opacidad del indicador" aria-valuetext={`${opacityDraft} %`} disabled={busy} onChange={event => setOpacityDraft(Number(event.target.value))}/><output>{opacityDraft} %</output><button className="soft-button" type="button" disabled={busy || opacityDraft === p.settings.overlay_opacity} onClick={() => patch({ overlay_opacity: opacityDraft })}>Aplicar</button></div></Row>
        </div>
      </section>

      <section className="vt-settings-page" hidden={category !== "data"}>
        <h1>Datos y app</h1><p className="vt-settings-lead">Inicio de Windows, historial y herramientas locales.</p>
        <h2 className="vt-settings-group-label">APLICACIÓN</h2><div className="surface settings-card">
          <Row title="Iniciar oculto" hint="Vozbit permanece accesible desde la bandeja.">{toggle("start_hidden", "Iniciar oculto")}</Row>
          <Row title="Iniciar con Windows">{toggle("launch_at_startup", "Iniciar con Windows")}</Row>
        </div>
        <h2 className="vt-settings-group-label">HISTORIAL</h2><div className="surface settings-card">
          <Row title="Guardar transcripciones" hint="Solo en este equipo.">{toggle("auto_save", "Guardar transcripciones")}</Row>
          <Row title="Entradas recientes" hint="Los favoritos se conservan aparte."><select disabled={busy} aria-label="Límite de historial" value={p.settings.history_limit} onChange={event => patch({ history_limit: Number(event.target.value) })}>{[20, 50, 100, 250, 500].map(value => <option key={value} value={value}>{value}</option>)}</select></Row>
          <Row title="Reprocesar último audio" hint="Disponible solo durante esta sesión. No vuelve a insertar en otra app."><button className="soft-button" type="button" disabled={!hasAudio || busy} onClick={() => void action(p.onReprocess, "Reprocesado")}>Reprocesar</button></Row>
        </div>
        <details className="settings-extra"><summary>Diagnóstico local <span>Avanzado</span></summary><div className="surface settings-card" aria-label="Diagnóstico local sin contenido del dictado">
          <Row title="Atajo global"><span className="diagnostic-value">{p.shortcut?.registered ? `Activo: ${p.shortcut.shortcut}` : "No registrado; cambia la combinación"}</span></Row>
          <Row title="Modelo y voz"><span className="diagnostic-value">{(p.settings.transcription_engine === "parakeet_ultra" ? parakeetReady : whisperReady) ? "Modelo y detector listos" : "Revisar modelo o detector"}</span></Row>
          <Row title="Último dictado" hint="Solo métricas de esta sesión; aquí no se muestra audio ni texto."><span className="diagnostic-value">{p.lastResult ? `${p.lastResult.captured_seconds.toFixed(1)} s de audio · ${(p.lastResult.post_release_ms / 1000).toFixed(1)} s desde soltar · ${(p.lastResult.transcript.inference_ms / 1000).toFixed(1)} s de motor · ${p.lastResult.prefetched_segments} fragmentos anticipados · ${p.lastResult.dropped_chunks} fragmentos perdidos` : "Sin dictados en esta sesión"}</span></Row>
        </div></details>
        <p className="vt-privacy-note">Todo permanece en este equipo. El último audio se conserva en memoria para reprocesarlo y desaparece al salir.</p>
      </section>

      <section className="vt-settings-page" hidden={category !== "help"}>
        <h1>Ayuda y acerca de</h1><p className="vt-settings-lead">Guía breve y datos de esta aplicación.</p>
        <div className="surface settings-card">
          <Row title="Cómo dictar" hint="Coloca el cursor en un campo, usa el atajo, habla y termina según tu modo de grabación."><button className="soft-button" type="button" onClick={p.onHelp}>Abrir ayuda</button></Row>
          <Row title="Tutorial" hint="Repite el recorrido de inicio."><button className="soft-button" type="button" onClick={p.onRepeatTutorial}>Repetir tutorial</button></Row>
          <Row title="Acerca de Vozbit" hint="Versión, privacidad y componentes locales."><button className="soft-button" type="button" onClick={p.onAbout}>Ver información</button></Row>
        </div>
      </section>
    </div>
  </div>;
}
