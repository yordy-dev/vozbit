export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
  sample_rate: number;
  channels: number;
  sample_format: string;
}

export interface MicLevel {
  rms: number;
  peak: number;
}

export interface AudioTestResult {
  device_name: string;
  input_sample_rate: number;
  output_sample_rate: number;
  channels: number;
  sample_format: string;
  input_frames: number;
  output_frames: number;
  duration_seconds: number;
  rms: number;
  peak: number;
  dropped_chunks: number;
  saved_path: string | null;
}

export interface ModelStatus {
  name: string;
  available: boolean;
  sha256_ok: boolean;
  engine_available: boolean;
  size_bytes: number;
  vad_available: boolean;
  vad_sha256_ok: boolean;
  parakeet_available: boolean;
  parakeet_sha256_ok: boolean;
  parakeet_engine_available: boolean;
  parakeet_vad_sha256_ok: boolean;
  parakeet_size_bytes: number;
}

export interface TranscriptionResult {
  text: string;
  requested_language: string;
  audio_seconds: number;
  inference_ms: number;
  model_name: string;
}

export interface DictationResult {
  transcript: TranscriptionResult;
  captured_seconds: number;
  dropped_chunks: number;
  dictionary_changes: number;
  dictionary_warning: string | null;
  delivery: { sent: boolean; reason: string | null };
  post_release_ms: number;
  prefetched_segments: number;
  streaming_used: boolean;
}

export interface HistoryItem {
  id: number;
  created_at_ms: number;
  text: string;
  language: string;
  audio_seconds: number;
  favorite: boolean;
}

export interface ShortcutStatus {
  shortcut: string;
  registered: boolean;
  error: string | null;
}

export interface DictionaryEntry {
  term: string;
  replacement: string;
  source: string;
  enabled: boolean;
}

export interface DictionaryPreview {
  text: string;
  changes: number;
}

export interface AppSettings {
  tutorial_version: number;
  learn_corrections: boolean;
  language: "auto" | "es" | "en";
  transcription_engine: "whisper" | "parakeet_ultra";
  preferred_device_id: string;
  auto_insert: boolean;
  auto_save: boolean;
  append_trailing_space: boolean;
  text_correction: boolean;
  format_numbers: boolean;
  math_symbols: boolean;
  push_to_talk: boolean;
  overlay_position: "top_center" | "center" | "bottom_center";
  overlay_opacity: number;
  start_hidden: boolean;
  launch_at_startup: boolean;
  history_limit: number;
}
