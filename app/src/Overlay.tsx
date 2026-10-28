import { useEffect, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { MicLevel } from "./types";
import { VoiceTextMark } from "./Icons";
import "./overlay-refined.css";

const BAR_COUNT = 24;

export default function Overlay() {
  const [phase, setPhase] = useState("idle");
  const [hold, setHold] = useState(false);
  const [message, setMessage] = useState("");
  const [opacity, setOpacity] = useState(100);
  const [levels, setLevels] = useState<number[]>(() => Array(BAR_COUNT).fill(0.08));
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    if (phase === "recording" || phase === "idle" || phase === "error") {
      setProgress(0);
      return;
    }
    if (phase === "done") { setProgress(100); return; }
    if (phase === "completing") {
      const started = performance.now();
      setProgress(94);
      const timer = window.setInterval(() => {
        const elapsed = (performance.now() - started) / 1000;
        setProgress(Math.min(99, 94 + 5 * (1 - Math.exp(-elapsed / 0.35))));
      }, 80);
      return () => window.clearInterval(timer);
    }
    if (phase !== "processing") return;
    const started = performance.now();
    setProgress(3);
    // The engines do not report per-token progress. This is an estimate that
    // never reaches the end until the backend confirms delivery.
    const timer = window.setInterval(() => {
      const elapsed = (performance.now() - started) / 1000;
      setProgress(Math.min(90, 3 + 87 * (1 - Math.exp(-elapsed / 4.5))));
    }, 80);
    return () => window.clearInterval(timer);
  }, [phase]);

  useEffect(() => {
    let active = true;
    const unsubscribers: Array<() => void> = [];
    void listen<string>("dictation://state", (event) => {
      if (!active) return;
      setPhase(event.payload);
    }).then((unlisten) => { if (active) unsubscribers.push(unlisten); else unlisten(); });
    void listen<{hold:boolean;message?:string}>("overlay://config", ({payload}) => {
      setHold(payload.hold); setMessage(payload.message ?? "");
    }).then(fn => { if(active) unsubscribers.push(fn); else fn(); });
    void listen<string>("overlay://message", ({payload}) => {
      if (active) setMessage(payload);
    }).then(fn => { if(active) unsubscribers.push(fn); else fn(); });
    void listen<number>("overlay://opacity", ({payload}) => {
      if (active) setOpacity(payload);
    }).then(fn => { if(active) unsubscribers.push(fn); else fn(); });
    void invoke<{overlay_opacity:number}>("settings_get").then(settings => {
      if (active) setOpacity(settings.overlay_opacity);
    }).catch(() => undefined);
    void listen<MicLevel>("audio://mic-level", (event) => {
      if (!active) return;
      const next = Math.min(1, Math.max(0.06, Math.sqrt(event.payload.rms) * 1.8));
      setLevels((current) => [...current.slice(1), next]);
    }).then((unlisten) => { if (active) unsubscribers.push(unlisten); else unlisten(); });
    return () => { active = false; unsubscribers.forEach((unlisten) => unlisten()); };
  }, []);

  const statusText = phase === "error" ? message || "Falló la transcripción" : phase === "processing" ? "Procesando localmente; avance visual estimado" : phase === "completing" ? "Transcripción lista; enviando texto" : phase === "done" ? "Texto entregado" : phase === "idle" ? "Listo para grabar" : hold ? "Suelta el atajo para transcribir" : "Pulsa el atajo para transcribir";
  return <div className={`overlay-shell state-${phase}`} style={{"--overlay-opacity": opacity / 100} as CSSProperties} role="status" aria-live="polite" aria-label={statusText} title={statusText} onMouseDown={event => { if(event.button === 0) void invoke("overlay_drag"); }}>
    <div className="overlay-brand"><VoiceTextMark size={20} className="overlay-mark"/></div>
    <div className="overlay-divider" aria-hidden="true"/>
    {phase === "error" ? <div className="overlay-error-message"><span className="overlay-error-icon" aria-hidden="true">!</span><span>{statusText}</span></div> :
      <div className="overlay-wave" role={phase === "recording" ? "meter" : undefined} aria-label={phase === "recording" ? "Nivel de micrófono" : undefined} aria-valuemin={phase === "recording" ? 0 : undefined} aria-valuemax={phase === "recording" ? 100 : undefined} aria-valuenow={phase === "recording" ? Math.round((levels.at(-1) ?? 0) * 100) : undefined} aria-hidden={phase !== "recording"}>
        {levels.map((level, index) => {
          const reached = (index + 1) / BAR_COUNT * 100 <= progress;
          const head = Math.abs((index + 0.5) / BAR_COUNT * 100 - progress) < 8;
          return <i key={index} className={phase === "processing" || phase === "completing" || phase === "done" ? `${reached ? "wave-reached" : ""} ${head ? "wave-head" : ""}` : undefined} style={{ "--bar-height": `${Math.max(3, level * 28)}px`, "--bar-index": index } as CSSProperties}/>;
        })}
      </div>}
  </div>;
}
