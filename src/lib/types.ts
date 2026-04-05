// ─── Core domain types ────────────────────────────────────────────────────────
// Shaped to match the canonical DB schema in CONTEXT.md.
// file_path is always a filesystem reference — PDFs are never copied.

export interface Project {
  id: string;
  name: string;
  description?: string;
  system_note?: string;
  created_at: number;
  updated_at: number;
}

export interface Document {
  id: string;
  project_id: string;
  file_path: string; // reference only — never copied
  title: string;
  page_count?: number;
  ocr_layout?: OcrBlock[];
  created_at: number;
}

export interface OcrBlock {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  page: number; // 0-indexed
  source: string;
}

export interface Thread {
  id: string;
  document_id: string;
  page_number: number;  // 0-indexed
  anchor_start?: number; // char offset in text_cache
  anchor_end?: number;
  anchor_quote: string;
  summary?: string;
  created_at: number;
  updated_at: number;
}

export interface Message {
  id: string;
  thread_id: string;
  role: 'user' | 'assistant';
  content: string;
  created_at: number;
}

export interface KbEntry {
  id: string;
  project_id: string;
  concept: string;
  summary: string;
  source_thread_id?: string;
  created_at: number;
  updated_at: number;
}

// ─── UI-layer types ───────────────────────────────────────────────────────────

export interface TextSelection {
  text: string;
  pageNumber: number; // 0-indexed
  anchorStart?: number;
  anchorEnd?: number;
}

export interface AppSettings {
  provider: 'ollama' | 'anthropic' | 'openai_compatible';
  ollama_base_url: string;
  ollama_model: string;
  anthropic_api_key?: string;
  openai_api_key?: string;
  openai_compatible_base_url?: string;
  context_window_pages: number;
  summarize_after_n_messages: number;
}
