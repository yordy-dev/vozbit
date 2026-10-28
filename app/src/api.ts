import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, AudioDevice, AudioTestResult, DictionaryEntry, DictionaryPreview, DictationResult, HistoryItem, MicLevel, ModelStatus, ShortcutStatus, TranscriptionResult } from "./types";

export const inDesktop = isTauri();

export async function listInputDevices(): Promise<AudioDevice[]> {
  return invoke<AudioDevice[]>("audio_list_input_devices");
}

export async function testInputDevice(deviceId: string | null, saveWav: boolean): Promise<AudioTestResult> {
  return invoke<AudioTestResult>("audio_test_input_device", {
    deviceId: deviceId === "default" ? null : deviceId,
    saveWav,
  });
}

export async function listenMicLevel(handler: (level: MicLevel) => void): Promise<UnlistenFn> {
  return listen<MicLevel>("audio://mic-level", (event) => handler(event.payload));
}

export async function getModelStatus(): Promise<ModelStatus> {
  return invoke<ModelStatus>("model_status");
}

export async function transcribeDiagnostic(path: string, language: "auto" | "es" | "en"): Promise<TranscriptionResult> {
  return invoke<TranscriptionResult>("transcription_transcribe_diagnostic", { path, language });
}

export async function startDictation(deviceId: string, deliveryRequested: boolean, language: "auto" | "es" | "en"): Promise<{ max_seconds: number }> {
  return invoke<{ max_seconds: number }>("dictation_start", { deviceId: deviceId === "default" ? null : deviceId, deliveryRequested, language });
}

export async function stopDictation(language: "auto" | "es" | "en"): Promise<DictationResult> {
  return invoke<DictationResult>("dictation_stop", { language });
}

export async function cancelDictation(showError = false): Promise<void> {
  return invoke<void>("dictation_cancel", { showError });
}

export async function reprocessDictation(language: "auto" | "es" | "en"): Promise<DictationResult> {
  return invoke<DictationResult>("dictation_reprocess", { language });
}

export async function getLastDictation(): Promise<DictationResult | null> {
  return invoke<DictationResult | null>("dictation_last_result");
}

export async function listenDictationError(handler: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("dictation://error", (event) => handler(event.payload));
}

export async function saveHistory(item: DictationResult): Promise<number> {
  return invoke<number>("history_save", {
    text: item.transcript.text,
    language: item.transcript.requested_language,
    audioSeconds: item.captured_seconds,
  });
}

export async function listHistory(query = ""): Promise<HistoryItem[]> {
  return invoke<HistoryItem[]>("history_list", { query });
}

export async function setHistoryFavorite(id: number, favorite: boolean): Promise<void> {
  return invoke<void>("history_set_favorite", { id, favorite });
}

export async function deleteHistory(id: number): Promise<void> {
  return invoke<void>("history_delete", { id });
}

export async function getShortcutStatus(): Promise<ShortcutStatus> {
  return invoke<ShortcutStatus>("shortcut_status");
}

export async function listenShortcutState(handler: (state: "pressed" | "released") => void): Promise<UnlistenFn> {
  return listen<"pressed" | "released">("shortcut://state", (event) => handler(event.payload));
}

export async function listenShortcutToggle(handler: () => void): Promise<UnlistenFn> {
  return listen("shortcut://toggle", handler);
}

export async function listDictionary(): Promise<DictionaryEntry[]> {
  return invoke<DictionaryEntry[]>("dictionary_list");
}

export async function upsertDictionary(term: string, replacement: string): Promise<void> {
  return invoke<void>("dictionary_upsert", { term, replacement });
}

export async function deleteDictionary(term: string): Promise<void> {
  return invoke<void>("dictionary_delete", { term });
}

export async function previewDictionary(text: string): Promise<DictionaryPreview> {
  return invoke<DictionaryPreview>("dictionary_preview", { text });
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("settings_get");
}

export async function patchSettings(patch: Partial<AppSettings>): Promise<void> {
  return invoke<void>("settings_patch", { patch });
}

export async function updateShortcut(shortcut: string): Promise<ShortcutStatus> {
  return invoke<ShortcutStatus>("shortcut_update", { shortcut });
}
