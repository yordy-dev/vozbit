import { useEffect, useState } from "react";
import { clearHistory, deleteHistory, inDesktop, listHistory, setHistoryFavorite } from "./api";
import { Icon } from "./Icons";
import type { HistoryItem } from "./types";
import "./history.css";

function errorText(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function HistoryPanel({ revision }: { revision: number }) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [deleting, setDeleting] = useState<number | null>(null);
  const [clearMode, setClearMode] = useState<"recent" | "all" | null>(null);
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    if (!inDesktop) return;
    let active = true;
    setLoading(true);
    const timer = window.setTimeout(() => {
      void listHistory(query).then((found) => {
        if (active) { setItems(found); setError(""); }
      }).catch((reason: unknown) => {
        if (active) setError(errorText(reason));
      }).finally(() => { if (active) setLoading(false); });
    }, 160);
    return () => { active = false; window.clearTimeout(timer); };
  }, [query, revision]);

  async function copy(item: HistoryItem) {
    try {
      await navigator.clipboard.writeText(item.text);
      setMessage("Texto copiado al portapapeles.");
      setError("");
    } catch (reason) { setError(`No se pudo copiar: ${errorText(reason)}`); }
  }

  async function toggleFavorite(item: HistoryItem) {
    try {
      await setHistoryFavorite(item.id, !item.favorite);
      setItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, favorite: !entry.favorite } : entry));
      setMessage(item.favorite ? "Quitado de favoritos." : "Añadido a favoritos.");
    } catch (reason) { setError(errorText(reason)); }
  }

  async function remove(item: HistoryItem) {
    if (deleting !== item.id) { setDeleting(item.id); return; }
    try {
      await deleteHistory(item.id);
      setItems((current) => current.filter((entry) => entry.id !== item.id));
      setMessage("Transcripción eliminada de este equipo.");
      setError("");
    } catch (reason) { setError(errorText(reason)); }
    finally { setDeleting(null); }
  }

  async function clear(includeFavorites: boolean) {
    const mode = includeFavorites ? "all" : "recent";
    if (clearMode !== mode) { setClearMode(mode); return; }
    if (clearing) return;
    setClearing(true);
    try {
      const removed = await clearHistory(includeFavorites);
      setItems((current) => includeFavorites ? [] : current.filter((item) => item.favorite));
      setMessage(`${removed} transcripci${removed === 1 ? "ón eliminada" : "ones eliminadas"} de este equipo.`);
      setError("");
    } catch (reason) { setError(errorText(reason)); }
    finally { setClearMode(null); setClearing(false); }
  }

  return <section className="recent-section" aria-labelledby="recent-title">
    <div className="recent-tools">
      <h2 id="recent-title">Transcripciones recientes</h2>
      <div className="history-toolbar">{(items.length > 0 || query.length > 0) && <input aria-label="Buscar transcripciones" type="search" value={query} onChange={(event) => setQuery(event.target.value)} maxLength={200} placeholder="Buscar" />}{items.length > 0 && <details onToggle={(event) => { if (!(event.currentTarget as HTMLDetailsElement).open) setClearMode(null); }}><summary>Limpiar</summary><div><button type="button" disabled={clearing} className={clearMode === "recent" ? "confirm" : ""} onClick={() => void clear(false)}>{clearing && clearMode === "recent" ? "Eliminando…" : clearMode === "recent" ? "Confirmar no favoritos" : "Eliminar no favoritos"}</button><button type="button" disabled={clearing} className={clearMode === "all" ? "confirm" : ""} onClick={() => void clear(true)}>{clearing && clearMode === "all" ? "Eliminando…" : clearMode === "all" ? "Confirmar todo" : "Eliminar todo"}</button></div></details>}</div>
    </div>
    {loading && items.length === 0 ? <div className="history-empty" role="status">Cargando tus transcripciones…</div> : null}
    {!loading && items.length === 0 && <div className="history-empty"><p>{query ? "No hay coincidencias." : "Todavía no hay transcripciones."}</p><span>{query ? "Prueba con otra palabra." : "Pulsa tu atajo global en cualquier aplicación y habla; el texto aparecerá aquí cuando lo guardes."}</span></div>}
    {error && <div className="error" role="alert">{error}</div>}
    {message && <p className="inline-message" role="status">{message}</p>}
    <div className="history-list">{items.map((item) => <article className="history-item" key={item.id}>
      <button className="history-copy-area" type="button" onClick={() => void copy(item)} title="Copiar transcripción">
        <div className="history-meta"><time dateTime={new Date(item.created_at_ms).toISOString()}>{new Date(item.created_at_ms).toLocaleString("es-PE", { dateStyle: "medium", timeStyle: "short" })}</time><span>{item.audio_seconds.toFixed(1)} s</span></div>
        <p>{item.text}</p><span className="copy-hint"><Icon name="copy" size={14}/> Haz clic para copiar</span>
      </button>
      <div className="history-actions"><button type="button" className={item.favorite ? "active" : ""} onClick={() => void toggleFavorite(item)} aria-label={item.favorite ? "Quitar de favoritos" : "Añadir a favoritos"}><Icon name="star" size={16}/></button><button type="button" className={deleting === item.id ? "confirm" : ""} onClick={() => void remove(item)} aria-label={deleting === item.id ? "Confirmar eliminación" : "Eliminar transcripción"}><Icon name="trash" size={16}/>{deleting === item.id && <span>Confirmar</span>}</button></div>
    </article>)}</div>
  </section>;
}
