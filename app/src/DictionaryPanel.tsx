import { useEffect, useRef, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { deleteDictionary, inDesktop, listDictionary, previewDictionary, upsertDictionary } from "./api";
import type { DictionaryEntry, DictionaryPreview } from "./types";
import "./dictionary.css";

function errorText(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function DictionaryPanel({ learningEnabled, onToggleLearning }: { learningEnabled: boolean; onToggleLearning: (enabled: boolean) => Promise<void> }) {
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [term, setTerm] = useState("");
  const [replacement, setReplacement] = useState("");
  const [example, setExample] = useState("");
  const [preview, setPreview] = useState<DictionaryPreview | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [learningBusy, setLearningBusy] = useState(false);
  const [query,setQuery]=useState("");
  const [suggestions,setSuggestions]=useState<DictionaryEntry[]>([]);
  const termInputRef = useRef<HTMLInputElement>(null);
  async function refresh(){const [saved,pending]=await Promise.all([listDictionary(),invoke<DictionaryEntry[]>("dictionary_suggestions")]);setEntries(saved);setSuggestions(pending);setError("");}
  async function review(item:DictionaryEntry,accept:boolean){try{await invoke('dictionary_review',{term:item.term,replacement:item.replacement,accept});await refresh();setMessage(accept?'Corrección aprendida. Puedes desactivarla o eliminarla.':'Sugerencia descartada.');}catch(e){setError(String(e));}}
  async function enable(item:DictionaryEntry){try{await invoke('dictionary_set_enabled',{term:item.term,enabled:!item.enabled});await refresh();}catch(e){setError(String(e));}}

  useEffect(() => {
    if (!inDesktop) return;
    void refresh().catch((reason: unknown) => setError(errorText(reason)));
    let active=true;let cleanup:(()=>void)|undefined;
    void listen('learning://updated',()=>void refresh().catch((reason: unknown) => setError(`No se pudo actualizar el diccionario: ${errorText(reason)}`))).then(fn=>{if(active)cleanup=fn;else fn();});
    return()=>{active=false;cleanup?.();};
  }, []);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true); setError(""); setMessage("");
    try {
      await upsertDictionary(term, replacement.trim() || term.trim());
      setEntries(await listDictionary());
      setMessage("Corrección guardada. Se aplicará a los próximos dictados.");
      setTerm(""); setReplacement(""); setPreview(null);
    } catch (reason) { setError(errorText(reason)); }
    finally { setBusy(false); }
  }

  async function remove(item: DictionaryEntry) {
    if (deleting !== item.term) { setDeleting(item.term); return; }
    try {
      await deleteDictionary(item.term);
      setEntries((current) => current.filter((entry) => entry.term !== item.term));
      setMessage("Corrección eliminada."); setError(""); setPreview(null);
    } catch (reason) { setError(errorText(reason)); }
    finally { setDeleting(null); }
  }

  async function runPreview() {
    setError("");
    try { setPreview(await previewDictionary(example)); }
    catch (reason) { setError(errorText(reason)); }
  }

  async function toggleLearning() {
    setLearningBusy(true); setError(""); setMessage("");
    try {
      await onToggleLearning(!learningEnabled);
      setMessage(learningEnabled ? "Aprendizaje desactivado." : "Aprendizaje activado. Las correcciones claras del último dictado se guardarán solas.");
    } catch (reason) { setError(errorText(reason)); }
    finally { setLearningBusy(false); }
  }

  const visibleEntries = entries.filter(i=>(i.term+' '+i.replacement).toLowerCase().includes(query.toLowerCase())).sort((a,b)=>Number(b.source==='learned')-Number(a.source==='learned')||a.term.localeCompare(b.term,'es'));

  return <>
    <header className="page-header"><h1>Diccionario</h1><p>Guarda nombres y términos que dictas a menudo. Revisa aquí las correcciones detectadas en otros editores.</p><div className="page-rule"><span /></div></header>
    <section className="surface dictionary-surface">
      <div className="dictionary-intro"><div><h2>Añadir término</h2><p>Escribe cómo apareció y cómo debe quedar. La corrección se aplicará a futuros dictados.</p></div></div>
      <div className="dictionary-learning"><div><strong>Aprender al corregir</strong><p>Con «Escribir en el campo activo», observa hasta 30 segundos ese campo si Windows permite leerlo. Una corrección clara de una palabra se guarda sola; las ambiguas pasan a revisión. No observa contraseñas ni otros campos.</p></div><label><input type="checkbox" checked={learningEnabled} disabled={!inDesktop || learningBusy} onChange={() => void toggleLearning()} aria-label="Aprender automáticamente de mis correcciones"/><span>{learningEnabled ? "Activado" : "Desactivado"}</span></label></div>
      <form className="dictionary-form" onSubmit={(event) => void save(event)}>
        <input ref={termInputRef} data-tour="dictionary-form" aria-label="Palabra o texto reconocido" value={term} onChange={(event) => setTerm(event.target.value)} maxLength={80} required placeholder="Agregar una palabra" />
        <span aria-hidden="true">→</span>
        <input aria-label="Corrección exacta opcional" value={replacement} onChange={(event) => setReplacement(event.target.value)} maxLength={80} placeholder="Corrección exacta (opcional)" />
        <button className="pill-button" type="submit" disabled={!inDesktop || busy}>{busy ? "Guardando…" : "Agregar"}</button>
      </form>
      <div className="dictionary-divider" />
      <p className="section-label">TÉRMINOS Y CORRECCIONES GUARDADAS</p>
      <div className="dictionary-toolbar"><input type="search" aria-label="Buscar palabras" placeholder="Buscar palabras y correcciones" value={query} onChange={e=>setQuery(e.target.value)}/></div>
      {entries.length === 0 ? <p className="dictionary-empty">Todavía no hay palabras guardadas.</p> : visibleEntries.length === 0 ? <p className="dictionary-empty">No hay coincidencias. Prueba otra búsqueda.</p> : <div className="learning-list">{visibleEntries.map(item=><div className="learning-item" key={item.term}><strong>{item.term===item.replacement?item.term:`${item.term} → ${item.replacement}`}</strong><small> · {item.source==='learned'?'Aprendida automáticamente':'Manual'} · {item.enabled?'Activa':'Desactivada'}</small><div><button className="soft-button" onClick={()=>{setTerm(item.term);setReplacement(item.replacement);setMessage('Edita la corrección en el formulario superior.');termInputRef.current?.scrollIntoView({behavior:'smooth',block:'center'});termInputRef.current?.focus();}}>Editar</button><button className="soft-button" onClick={()=>void enable(item)}>{item.enabled?'Desactivar':'Activar'}</button><button className="soft-button" onClick={()=>void remove(item)}>{deleting===item.term?'Confirmar eliminación':item.source==='learned'?'Deshacer aprendizaje':'Eliminar'}</button></div></div>)}</div>}
      {error && <div className="error" role="alert">{error}</div>}
      {message && <p className="inline-message" role="status">{message}</p>}
    </section>
    {suggestions.length>0&&<section className="surface dictionary-preview"><h2>Por revisar</h2><p>Detectadas al corregir el texto recién dictado en otro campo. Las cifras y expresiones se guardan con contexto: confirma solo cambios que quieras repetir. Nunca se cambia globalmente un número aislado.</p><div className="learning-list">{suggestions.map(item=><div className="learning-item" key={item.term}><strong>{item.term} → {item.replacement}</strong><div><button className="soft-button" onClick={()=>void review(item,true)}>Aprender</button><button className="soft-button" onClick={()=>void review(item,false)}>Descartar</button></div></div>)}</div></section>}
    <details className="surface dictionary-preview vt-dictionary-test"><summary>Probar una frase</summary><p>Comprueba el resultado sin grabar ni modificar el historial.</p><textarea value={example} onChange={(event) => { setExample(event.target.value); setPreview(null); }} maxLength={10000} rows={3} placeholder="Escribe aquí una frase con alguno de tus términos"/><button className="outline-button" type="button" disabled={!inDesktop || !example} onClick={() => void runPreview()}>Ver corrección</button>{preview && <div className="preview-result" role="status"><span>{preview.changes} cambio{preview.changes === 1 ? "" : "s"}</span><p>{preview.text}</p></div>}</details>
  </>;
}
