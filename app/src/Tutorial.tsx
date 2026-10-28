import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { VoiceTextLogo } from "./Icons";
import "./tutorial.css";

type TourView = "home" | "settings" | "dictionary";
type TourStep = { view: TourView; target?: string; title: string; text: string };
type Box = { left: number; top: number; width: number; height: number; bottom: number };

const steps: TourStep[] = [
  { view: "home", target: '[data-tour="dictation"]', title: "Dicta en otro editor", text: "Pon el cursor en otro editor y usa tu atajo. La prueba de Inicio no inserta texto." },
  { view: "settings", target: '[data-tour="shortcut"]', title: "Tu atajo de dictado", text: "Púlsalo dos veces, o mantén las teclas y suéltalas. La mini indica grabación y procesamiento." },
  { view: "settings", target: '[data-tour="delivery"]', title: "Dónde aparece el texto", text: "Elige «Escribir en el campo activo» para dictar fuera de Vozbit. El modo manual deja el texto en Inicio." },
  { view: "dictionary", target: '[data-tour="dictionary-form"]', title: "Enseña tus palabras", text: "Añade términos y correcciones. Aprender desde otros editores requiere activación y revisión." },
  { view: "home", title: "Sigue en la bandeja", text: "Cerrar la ventana no detiene Vozbit. Clic izquierdo para abrir; clic derecho para Ayuda o Salir." },
];

function placeCard(box: Box | null, width: number, height: number) {
  const margin = 12;
  const gap = 12;
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;
  const clamp = (value: number, limit: number) => Math.max(margin, Math.min(value, Math.max(margin, limit - margin)));
  if (!box) return { left: clamp((viewportWidth - width) / 2, viewportWidth - width), top: clamp((viewportHeight - height) / 2, viewportHeight - height) };
  const candidates = [
    { left: box.left + (box.width - width) / 2, top: box.bottom + gap },
    { left: box.left + (box.width - width) / 2, top: box.top - height - gap },
    { left: box.left - width - gap, top: box.top + (box.height - height) / 2 },
    { left: box.left + box.width + gap, top: box.top + (box.height - height) / 2 },
  ].map((candidate) => ({ left: clamp(candidate.left, viewportWidth - width), top: clamp(candidate.top, viewportHeight - height) }));
  const overlap = (candidate: { left: number; top: number }) => {
    const x = Math.max(0, Math.min(candidate.left + width, box.left + box.width) - Math.max(candidate.left, box.left));
    const y = Math.max(0, Math.min(candidate.top + height, box.bottom) - Math.max(candidate.top, box.top));
    return x * y;
  };
  return candidates.reduce((best, candidate) => overlap(candidate) < overlap(best) ? candidate : best);
}

export function Tutorial({ navigate, finish }: { navigate: (view: TourView) => void; finish: () => void }) {
  const [started, setStarted] = useState(false);
  const [step, setStep] = useState(0);
  const [box, setBox] = useState<Box | null>(null);
  const [size, setSize] = useState({ width: 318, height: 180 });
  const card = useRef<HTMLDivElement>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const welcome = useRef<HTMLDivElement>(null);
  const finishRef = useRef(finish);
  finishRef.current = finish;

  useEffect(() => {
    navigate(started ? steps[step].view : "home");
    if (!started || steps[step].view !== "settings") return;
    const frame = window.requestAnimationFrame(() => window.dispatchEvent(new CustomEvent("voicetext:settings-category", { detail: step === 2 ? "output" : "general" })));
    return () => window.cancelAnimationFrame(frame);
  }, [navigate, started, step]);
  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); finishRef.current(); }
      if (event.key !== "Tab") return;
      const focusable = [...(dialog.current?.querySelectorAll<HTMLElement>('button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]') ?? [])];
      if (!focusable.length) { event.preventDefault(); return; }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || !dialog.current?.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !dialog.current?.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
    };
    window.addEventListener("keydown", keydown);
    return () => { window.removeEventListener("keydown", keydown); previousFocus?.focus(); };
  }, []);
  useLayoutEffect(() => {
    if (started) card.current?.querySelector<HTMLElement>('h2')?.focus();
    else welcome.current?.querySelector<HTMLElement>('h2')?.focus();
  }, [started, step]);
  useLayoutEffect(() => {
    if (!started) { setBox(null); return; }
    const targetSelector = steps[step].target;
    const update = () => {
      const target = targetSelector ? document.querySelector(targetSelector) : null;
      if (target) {
        const before = target.getBoundingClientRect();
        if (before.top < 70 || before.bottom > window.innerHeight - 12) target.scrollIntoView({ block: "center", inline: "nearest" });
        const next = target.getBoundingClientRect();
        setBox((previous) => previous && previous.left === next.left && previous.top === next.top && previous.width === next.width && previous.height === next.height ? previous : { left: next.left, top: next.top, width: next.width, height: next.height, bottom: next.bottom });
      } else setBox(null);
      if (card.current) {
        const next = card.current.getBoundingClientRect();
        setSize((previous) => previous.width === next.width && previous.height === next.height ? previous : { width: next.width, height: next.height });
      }
    };
    update();
    const frame = window.requestAnimationFrame(update);
    const timer = window.setTimeout(update, 150);
    const lateTimer = window.setTimeout(update, 400);
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, { capture: true, passive: true });
    return () => { window.cancelAnimationFrame(frame); window.clearTimeout(timer); window.clearTimeout(lateTimer); window.removeEventListener("resize", update); window.removeEventListener("scroll", update, true); };
  }, [started, step]);

  if (!started) return <div className="tour-layer" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="tour-title" aria-describedby="tour-description"><div className="tour-scrim"/><div className="tour-welcome" ref={welcome}><button className="tour-close" type="button" aria-label="Omitir tutorial" onClick={finish}>×</button><span className="tour-eyebrow">BIENVENIDO A VOZBIT</span><div className="tour-mark"><VoiceTextLogo/></div><h2 id="tour-title" tabIndex={-1}>Tu voz, donde escribes</h2><p id="tour-description">Cinco pasos breves para dictar, elegir el atajo y enseñar tus palabras. No cambiaremos tus ajustes.</p><div className="tour-actions"><button className="tour-link" type="button" onClick={finish}>Entrar sin tutorial</button><button className="tour-primary" type="button" onClick={() => setStarted(true)}>Ver recorrido <span aria-hidden="true">→</span></button></div></div></div>;

  const current = steps[step];
  const placement = placeCard(box, Math.min(size.width, window.innerWidth - 24), size.height);
  return <div className="tour-layer" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="tour-title" aria-describedby="tour-description">
    {!box && <div className="tour-scrim"/>}
    {box && <div className="tour-focus" style={{ left: box.left - 4, top: box.top - 4, width: box.width + 8, height: box.height + 8 }}/>} 
    <div className="tour-card" ref={card} style={{ left: placement.left, top: placement.top }}>
      <div className="tour-card-top"><span>PASO {step + 1} DE {steps.length}</span><div className="tour-dots" aria-hidden="true">{steps.map((_, index) => <span key={index} className={index === step ? "active" : ""}/>)}</div><button className="tour-close" type="button" aria-label="Omitir tutorial" onClick={finish}>×</button></div>
      <h2 id="tour-title" tabIndex={-1}>{current.title}</h2><p id="tour-description">{current.text}</p>
      <div className="tour-actions"><button className="tour-link" type="button" onClick={finish}>Omitir</button><button className="tour-secondary" type="button" onClick={() => step ? setStep(step - 1) : setStarted(false)}>Anterior</button><button className="tour-primary" type="button" onClick={() => step === steps.length - 1 ? finish() : setStep(step + 1)}>{step === steps.length - 1 ? "Listo" : "Siguiente"} <span aria-hidden="true">→</span></button></div>
    </div>
  </div>;
}
