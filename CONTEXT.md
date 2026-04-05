# Scholium — Full Project Context (Updated)

> This document is the authoritative record of all design decisions, architectural
> choices, and product reasoning established during the full planning and scaffolding
> session. It supersedes any earlier version. It is intended to be read by Claude Code
> (or any collaborating AI assistant) before continuing development, so that no
> decision needs to be re-litigated and no context is lost.
>
> Last updated: after thread panel design and canvas selection architecture were finalized.

---

## 1. What Scholium Is

Scholium is a **macOS desktop PDF reader and academic reading companion** built with
Tauri (Rust backend + Svelte/TypeScript frontend). Its central insight is that
AI-assisted comprehension should be **spatially anchored to the text**, not floating
in a separate application.

The target user is an academic — graduate student, researcher, or serious independent
reader — who routinely grapples with dense technical texts (papers, textbooks,
monographs) and currently has to jump between multiple notes apps and AI chat
interfaces to resolve confusion. Scholium collapses that workflow into a single,
minimal, purpose-built tool.

### The Core Problem Being Solved

When a reader is stuck on a passage, they:
1. Switch to an AI chat interface (losing document context)
2. Paste the passage, explain their confusion, get a response
3. Return to the document — but the conversation is gone
4. On a future reading session, hit the same wall and repeat the entire process

Scholium solves this with **persistent, passage-anchored conversation threads**.
Every conversation is born attached to the specific text selection that prompted it,
lives in the margin of the document, collapses when not in use, and is immediately
available on every future visit to that passage. This directly addresses what we
called the **cold return problem**.

---

## 2. Product Philosophy — Mutual Distillation

This is the most important conceptual framing of the product and should inform every
design decision going forward.

Each conversation thread is simultaneously:
- The **user** building a clearer mental model of a concept
- The **agent** building a progressively refined summary of what *this user*
  understands, where they got confused, and how the resolution landed

This is not just a storage optimization — it is the **philosophical core of the app**.
The thread summary is the agent's "understanding" artifact. Over time, the
knowledgebase (KB) accumulates a jointly constructed record of what the user and
agent have worked through together. On cold return, the agent is not a blank slate —
it resumes a collaboration.

### Marketing Angle
The app genuinely becomes more valuable the longer it is used, because the KB grows
richer. This is an honest retention mechanic (not a dark pattern). Potential tagline
territory: *"Learn together."* or *"The more you read, the smarter it gets — about you."*

### Connection to Vygotsky's ZPD (North Star, v2+)
An agent that accumulates a model of the user's understanding is positioned to
calibrate its explanations to sit in the user's zone of proximal development —
the productive edge of what they can currently grasp. No static textbook can do this.
This is a v2/v3 research direction but should be kept as a north star, and is an
academically resonant idea that lands well with the target audience.

### Auto-generated Concept Map (Deferred, Principled)
The KB naturally accumulates concept summaries tied to passages. A read-only,
auto-generated concept graph is a compelling future artifact — but only as a
byproduct of normal use, never something the user curates. Do not build this
until the KB is rich enough to make it meaningful.

---

## 3. Competitive Landscape

| Tool | Gap |
|---|---|
| Elicit, Consensus | AI over papers, no PDF reader, no anchored persistent chats |
| Readwise Reader | Good PDF tool, some AI Q&A, not spatially anchored or persistent |
| Notion AI / Obsidian | Great KBs, but context-switching out of the reader |
| ChatPDF, Adobe Acrobat AI | Whole-document Q&A, no anchoring, no persistence, no projects |
| Highlights (macOS) | Nice PDF reader with annotation export, no AI |

The gap Scholium occupies: spatially anchored, persistent, project-scoped AI
conversations with a jointly maintained knowledge base.

---

## 4. Stack Decisions and Rationale

### Tauri (not Electron)
- Rust backend: native file I/O, PDF text extraction, SQLite, OCR — fast and memory-safe
- Smaller binary: ~8MB vs ~150MB (no bundled Chromium)
- Lower memory footprint: uses OS-native webview

The UI is still a webview — frontend rendering performance is identical to a browser
app. The gains are specifically in the backend.

### Frontend: SvelteKit + TypeScript + Vite
Fast, reactive framework using Svelte 5. TypeScript throughout — the IPC boundary between Rust and the
frontend is where type safety matters most.

### Styling: TailwindCSS
Utility-first, keeps component files self-contained.

### State Management: Svelte 5 Runes
Lightweight, zero-boilerplate reactive state management native to Svelte 5. Good fit for this app's state shape.

### PDF Rendering: PDF.js (Mozilla)
Rendered into a canvas element. Text selection natively utilizes the PDF.js HTML text layer. The backend intercepts and modifies PDF font and glyph geometries prior to frontend delivery to ensure the visual rendering perfectly aligns with the invisible HTML text layer, enabling clean sentence-level selection.

### Math Rendering: KaTeX
In the thread panel for AI responses. Non-negotiable for the target audience.
AI system prompt instructs models to use $...$ and $$...$$ notation.
Frontend uses marked (or similar markdown parser) + KaTeX for Svelte.

### Database: SQLite via rusqlite
- Synchronous, well-tested SQLite bindings for Rust
- Simple inline table creation at startup
- Foreign keys enforced

### HTTP Client: reqwest (async)
Streaming via bytes_stream(). Tokens forwarded to frontend via Tauri events
(ai://token, ai://done).

---

## 5. AI Provider Architecture

### Provider Abstraction

```rust
pub enum Provider {
    Ollama { base_url: String, model: String },
    Anthropic { api_key: String, model: String },
    OpenAiCompatible { base_url: String, api_key: String, model: String },
}
```

All three speak the OpenAI chat completions wire format. Only base_url and auth
header vary. Streaming and prompt logic is written once.

### Default: Ollama (local, offline, free)
Fresh install works offline, for free, immediately. Users who want frontier quality
opt into their own API key.

### Recommended Local Models

| Model | Size | Notes |
|---|---|---|
| llama3.1:8b | ~5GB | Strong general reasoning, good default |
| deepseek-r1:8b | ~5GB | Strong reasoning, good for technical/math |
| qwen2.5:7b | ~5GB | Notably strong at STEM and math |
| llama3.1:70b | ~40GB | Near-frontier, needs serious hardware |

### Settings Panel
```
AI Provider
  Local (Ollama) — no internet required   (default)
  Use my Anthropic key  [key input]
  Use my OpenAI key     [key input]
```

### Honest Caveat on Local Model Quality
Quality gap between local 8B models and frontier models is real and worst at the
hardest problems — exactly when users need help most. Be transparent about this in
onboarding. Surface a clear path to configuring a cloud key.

---

## 6. Aesthetic Direction

### Guiding Principle
The PDF and the conversation are the content. The chrome should functionally
disappear. The app should feel like sitting at a clean desk with a good book and a
knowledgeable colleague beside you — not like opening a SaaS tool.

### Color
- Background: warm off-white (#F7F4EF) — not pure white (harsh for long sessions),
  not dark mode as default. Psychologically reads as paper.
- Text/chrome: deep warm gray (#1C1917) — not pure black
- Accent: dusty terracotta or aged ochre — warm, scholarly, visually distinct without
  being jarring. Used for highlights, thread markers, interactive elements.
- Dark mode offered but not default.

### Typography
Two fonts:
- UI chrome (sidebar, labels, metadata): geometric and quiet — DM Sans, Instrument
  Sans, or Geist. Small, tracked out, muted weight.
- Thread panel / AI responses: readable serif — Lora, Source Serif 4, or Freight
  Text. Deliberate: AI produces discursive explanatory prose; a serif renders it the
  way an academic is used to reading. Also visually distinguishes the AI's voice from
  the UI chrome.

### Layout
- Two-panel split, ~60/40 PDF to thread panel. PDF gets more space.
- Thread panel attends to the document, does not compete with it.
- Generous internal padding in thread panel.
- Messages: not chat-app bubbles. Well-spaced prose with subtle left border or faint
  rule distinguishing user from assistant.

### Highlights and Thread Markers
- Semi-transparent warm amber rectangles drawn directly on the canvas overlay
- Thin vertical left border in accent color (the marginalia anchor feel)
- Collapsed thread indicators: small, unobtrusive, feel like marginalia —
  the visual tradition of handwritten notes in the margin of a physical text
- Not buttons or chips — something that belongs in the margin of a book

### What to Avoid
- Blue as accent (too generic, too software-y)
- Rounded corners everywhere (consumer app feel)
- Animations that are not directly functional
- Shadows and gradients (flat but warm)
- Dark mode as default

---

## 7. PDF Text Layer Glyph Alignment Architecture

PDF.js renders PDFs visually onto a canvas, then overlays a transparent HTML text
layer whose div positions are calculated from the PDF's internal coordinate system.
When a PDF's coordinate system is corrupted, or generated from a scan, those text layer
divs do not sit where the glyphs appear visually. This prevents clean sentence selections.

The solution: Dynamic PDF Glyph Realignment
Before feeding the PDF to the frontend, the Rust backend actively parses and reconstructs 
the PDF object tree, mathematically realigning embedded font bounding boxes, ligature 
encodings, and text matrices so that they map exactly 1:1 with standard reading order 
and visual pixel coordinates. 

This guarantees that the transparent HTML text layer rendered by PDF.js always sits 
perfectly on top of the text, enabling native browser text selection (e.g. partial sentences,
drag-to-highlight) without any custom canvas selection overlay.

---

## 8. Native HTML Selection Architecture — The Core UX Mechanism

### Core Insight
We rely entirely on standard HTML text selection provided by the browser and PDF.js. 
Since the backend ensures the PDF text layer is strictly aligned and reading-order-correct, 
the user simply selects text as they would on any webpage. Это allows for clean partial-sentence highlights.

### Selection Interaction
1. User selects text using the native mouse text-selection cursor on the HTML text layer.
2. Standard `window.getSelection()` captures the exact string (`anchor_quote`) and DOM offsets.
3. On mouse-up, if selection is not collapsed, a SelectionPopover appears ("Ask AI" / dismiss).
4. On confirm, the frontend computes the character offsets (`anchor_start` and `anchor_end`) 
   within the page or document text.
5. `invoke create_thread` is called with these offsets and the `anchor_quote`.
6. Highlight rendering is handled via standard DOM text range highlighting methods 
   (e.g., CSS Custom Highlight API or wrapping text nodes in `<mark>` elements).

---

## 9. OCR Architecture — Text Extraction Waterfall

At document load time, for any PDF with poor or no native text layer:

```
Step 1: pdf-extract on raw PDF
  text quality 'good' (>200 chars/page extracted) -> store text_cache, done
  quality 'poor' or 'none' -> continue

Step 2: mdls query for kMDItemTextContent (Spotlight cache)
  non-null result -> store text_cache, done
  null (not indexed or Spotlight disabled) -> continue

Step 3: macOS Vision OCR (VNRecognizeTextRequest) page by page
  show progress: "Enhancing text layer... page N of M"
  store text in text_cache permanently
  store bounding boxes in ocr_layout (JSON)
  never runs again for this document
```

### Text Layer Quality Detection
Run at load time. Check character count from first few pages via PDF.js getTextContent().
Surface a banner to the user if poor or none: "This PDF has a low-quality text layer.
Enhancing with OCR..." Never silently degrade.

### Spotlight Query (Step 2)
Use mdls subprocess: `mdls -name kMDItemTextContent <file_path>`
kMDItemTextContent is populated by Spotlight indexing, which includes OCR for scanned
documents. Free, instant, high quality. Works for any file the user has previously
opened on their Mac.

### Vision OCR (Step 3)
macOS Vision framework VNRecognizeTextRequest. Invoked via Swift helper subprocess
or objc2 Rust bindings. Returns word-level bounding boxes in normalized coordinates
(origin bottom-left — apply y-axis flip before storing).

Quality is very high, runs on Neural Engine on Apple Silicon (under 1 second per
page on M-series). Does not bundle Tesseract (~20MB). macOS-only, acceptable since
Scholium targets macOS first. Tesseract was explicitly rejected for v1.

### OCR Layout Storage
Bounding boxes stored as JSON in documents.ocr_layout:

```rust
pub struct OcrBlock {
    pub text: String,
    pub x: f64,      // normalized [0,1], origin top-left (y-flip applied)
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub page: i64,   // 0-indexed
}
```

Storage cost: ~300 pages x ~200 words x ~80 bytes = ~5MB per document. Acceptable.

### Text Resolution from Selection
When a text selection occurs on the frontend:
1. `window.getSelection().toString()` is captured directly as the `anchor_quote`.
2. Character offsets (`anchor_start` and `anchor_end`) are computed and stored.

### Re-Anchoring
Since the backend actively repairs the PDF text matrix, character offsets (`anchor_start`, `anchor_end`) 
serve as highly reliable anchors within the document. Text highlights are restored by reconstructing DOM Ranges 
using these exact offsets and the underlying HTML text layer.

---

## 10. Thread Panel — Design and Implementation

### Three States

Empty state (no thread selected):
- Subtle prompt: "Select any passage to begin a conversation"
- Collapsed list of all threads in current document, sorted by page number
- Each collapsed entry shows: page number, anchor quote truncated to one line,
  message count, last active timestamp

Active thread state:
- Thread header with anchor quote and optional summary
- Scrollable message list
- Input area pinned to bottom

KB panel state (toggled via icon in panel header):
- List of KB entries for current project
- Each entry: concept label (bold), summary text, source thread backlink

### Thread Header
The anchor quote is styled as a pull quote / blockquote — the thing that precipitated
the conversation. Warm left border, muted italic serif text, slightly inset.

If thread.summary is non-null, show a collapsed "What we established" section below
the anchor quote. One click expands it. This is the mutual distillation artifact made
directly visible — on cold return the user sees what was resolved before re-reading.

```tsx
<details>
  <summary>What we established</summary>
  <p>{thread.summary}</p>
</details>
```

Style: small, muted, uppercase tracking on the summary label. Expanded text in serif,
stone-500, with a faint left border.

### Message Rendering
Messages rendered as prose, not chat bubbles. No colored background pills.

User messages:
- No background, no bubble
- Slightly smaller text, stone-600
- Consistent alignment (decide left or right, do not mix)

Assistant messages:
- Serif font (Lora or Source Serif 4)
- Full markdown via react-markdown
- Math via rehype-katex — both inline $...$ and display $$...$$
- Generous top margin for separation from user turn
- Code blocks: monospace, very lightly tinted background, no heavy borders

### Streaming Display
While streaming:
- Tokens appear in StreamingBubble with same serif font and markdown styling
- Subtle blinking cursor at stream end
- KaTeX renders incrementally as complete math expressions are detected
  (watch for balanced $ delimiters before attempting render)
- On ai://done event: replace StreamingBubble with fully rendered MessageBubble

### Input Area
- Auto-growing textarea (up to ~5 lines, then scrolls)
- Enter to send, Shift+Enter for newline
- Send button with keyboard shortcut label
- Disabled and visually muted while streaming
- Placeholder: "Ask about this passage..."

### Collapsed Thread Indicators (in PDF margin)
When a thread exists on a page but is not active, show a marginalia-style indicator 
in the DOM margin corresponding to the vertical y-offset of the text selection:
- Small vertical amber line (2.5px wide, ~24px tall) floating on the left side.
- Tiny badge with message count if > 0
- On hover: tooltip showing first line of anchor_quote
- On click: open that thread in the thread panel
- DOM-based positioning relative to the text layer elements.

---

## 11. Context Stack (Prompt Assembly)

For each AI query, context assembled in this order (later = closer to model attention):

1. Project system_note — user-set domain context
2. Relevant KB entries — concepts previously worked through (keyword/recency in v1,
   semantic search in v2)
3. Document text window — +/- N pages around the anchor (N from settings, default 2),
   sliced from text_cache
4. Thread summary — rolling mutual understanding summary of prior conversation
5. Recent verbatim messages — last 8 (configurable)
6. Anchor quote — selected passage, explicitly flagged in system prompt
7. User's new message

System prompt instructs: be precise and rigorous, calibrate to user's apparent level,
use LaTeX notation, cite specific parts of the provided text.

---

## 12. Mutual Distillation Mechanism (Summarization Job)

After every completed AI response, async background job (non-blocking, tokio::spawn):

1. Load all messages for the thread
2. Only trigger if >= 4 messages (2 full exchanges)
3. Call complete_once() with summarization prompt — 2-4 sentences covering:
   what the user found confusing, the key insight or resolution, remaining open questions
4. Persist to threads.summary
5. Call complete_once() again — extract 3-6 word concept label
6. Upsert kb_entries row for the project

Uses complete_once() (non-streaming, max_tokens: 512). Does not block main response.
Does not consume streaming budget.

---

## 13. Storage Analysis and Mitigations

Conversation text alone is not a storage concern. Worst case: ~1.6MB for a heavily
annotated 500-page textbook. Even 10x that is trivial for SQLite.

Actual risks and mitigations:

| Risk | Mitigation |
|---|---|
| PDFs copied into app data | Never copy. Store file_path reference only. |
| AI context cached per thread | Never cache assembled context. Assemble at query time. |
| text_cache duplicated per thread | Stored once per document, shared across all threads. |
| OCR layout JSON | ~5MB per document. Acceptable. Stored in documents.ocr_layout. |
| Vector embeddings (v2) | ~4MB per document. Defer to v2. |
| Long thread verbatim history | Summarization distills old messages. Keep last N verbatim. |

---

## 14. Project / Session Management

### Two-Level Hierarchy
```
Workspace (implicit, per-installation)
  Projects
    Documents
      Threads (emerge from reading, not user-organized)
```
No third level. No folders within projects. No tags on documents in v1.

### What a Project Contains
- Name, optional description
- system_note — most important field. User's context-setting prompt for the agent.
  Actively prompted during onboarding. Example: "We are studying quantum error
  correction. Assume familiarity with stabilizer formalism but not fault tolerance."
- Document list (manually reorderable via display_order)
- Shared KB across all documents in the project
- last_active timestamp

### Project List View (Home Screen)
Each entry: name, document count, KB entry count (signals accumulated mutual
understanding — reinforces the distillation narrative), last active timestamp,
one-line description. Sorted by last active. No folders, no nesting, no tags in v1.

### Document Organization
Flat, manually reorderable. Each entry shows: title (editable), thread count,
page count, text layer quality indicator, last read page.

last_read_page stored and updated on scroll — opening a document returns to where
the user left off.

### Onboarding Flow (New Project)
Three-step dialogue, not a form:
```
What are you working on?
  [free text — name/description]

What should I know about your background with this material?
  [free text — seeds system_note]

Add your first document to get started.
  [file picker]
```
Second question does real work — seeds the agent's prior. Frame as "helping your
reading companion get oriented."

### Settings Boundary
Project-level: system_note, document list and order, (v2) preferred model per project
App-level: AI provider + keys, context window size, summarization threshold, UI prefs

Do not bleed app settings into projects. Projects must not contain credentials or
machine-specific preferences. They should be exportable in principle.

---

## 15. Database Schema (Canonical)

```sql
CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    system_note TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE documents (
    id             TEXT PRIMARY KEY,
    project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    file_path      TEXT NOT NULL,
    title          TEXT,
    text_cache     TEXT,
    ocr_layout     TEXT,
    page_count     INTEGER,
    last_read_page INTEGER DEFAULT 0,
    display_order  INTEGER DEFAULT 0,
    created_at     INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE threads (
    id           TEXT PRIMARY KEY,
    document_id  TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_number  INTEGER NOT NULL,
    anchor_start INTEGER,
    anchor_end   INTEGER,
    anchor_quote TEXT NOT NULL,
    summary      TEXT,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE messages (
    id          TEXT PRIMARY KEY,
    thread_id   TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content     TEXT NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE kb_entries (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    concept          TEXT NOT NULL,
    summary          TEXT NOT NULL,
    source_thread_id TEXT REFERENCES threads(id) ON DELETE SET NULL,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at       INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### Schema Changes from Original Version
- documents: added ocr_layout, last_read_page, display_order
- threads: `anchor_start` and `anchor_end` represent precise character offsets. Relying on them is safe since the backend guarantees PDF text coordinates are realigned and stable.

### Settings Defaults
```
provider                    -> "ollama"
ollama_base_url             -> "http://localhost:11434"
ollama_model                -> "llama3.1:8b"
anthropic_api_key           -> null
openai_api_key              -> null
openai_compatible_base_url  -> null
context_window_pages        -> 2
summarize_after_n_messages  -> 8
```

---

## 16. Rust Module Structure

```
src-tauri/src/
  main.rs
  lib.rs                  Tauri setup, plugin registration, all commands registered
  db.rs                   Database connection setup, inline schema creation, and queries
  pdf/
    mod.rs                extract(), assessQuality(), page_window(), OcrBlock
  ocr/
    mod.rs                get_spotlight_text(), invoke_vision_ocr(),
                          store_ocr_layout(), resolve_selection_text()
  ai/
    mod.rs                Provider enum, stream_completion(), complete_once()
    prompt.rs             PromptContext, build(), summarization_prompt()
    summarize.rs          run_summarization_job(), maybe_upsert_kb_entry()
  commands/
    mod.rs
    projects.rs           create, list, get, update, delete
    documents.rs          add (triggers extraction waterfall), list, get_text, remove
    threads.rs            create, list_for_document, get, delete
    selection.rs          resolve_selection_text (NormalizedRect -> anchor_quote)
    messages.rs           send_message (persist->stream->async summarize), list
    kb.rs                 list_entries, get_project_kb_summary
    settings.rs           get_settings, update_settings
```

### Key Crates
| Purpose | Crate |
|---|---|
| Async runtime | tokio (full) |
| SQLite | rusqlite (bundled) |
| HTTP client | reqwest (json + stream) |
| PDF text extraction | pdf-extract |
| Serialization | serde, serde_json |
| UUIDs | uuid (v4) |
| Error handling | anyhow |
| Async streaming | futures-util |
| Logging | tracing, tracing-subscriber |
| OCR (macOS) | Swift subprocess or objc2 (Vision framework) |

---

## 17. Frontend Module Structure

```
src/
  lib/
    types.ts              Project, Document, Thread, Message, KbEntry, AppSettings, etc.
    tauri.ts              typed invoke() wrappers for every Rust command
    stores/
      app.svelte.ts       Svelte 5 runes for app state, active document, threads, messages
    components/
      pdf/
        PdfViewer.svelte       outer container, page list, zoom controls, rendering (uses native DOM selection)
        SelectionPopover.svelte "Ask AI" / dismiss popover on selection complete
      thread/
        ThreadPanel.svelte     panel container, empty state, active thread, KB toggle
        ThreadHeader.svelte    anchor quote blockquote + "What we established" summary
        MessageList.svelte     scrollable message history
        MessageBubble.svelte   fully rendered markdown + KaTeX message
        StreamingBubble.svelte live token display, replaced on ai://done
        ThreadInput.svelte     auto-growing textarea, Enter to send
      project/
        ProjectSidebar.svelte  project/document list, nav
        DocumentList.svelte    documents with thread counts
        KbPanel.svelte         KB entries for active project
        ProjectOnboarding.svelte three-step dialogue for new project creation
      ui/                      Button, Input, Tooltip, Badge, etc.
  routes/
    +layout.svelte        root layout wrapper
    +page.svelte          main two-panel composition
```

---

## 18. Data Flow — Single Query End to End

```
1.  User selects text using native mouse drag on the HTML text layer
2.  onMouseUp -> window.getSelection() captures text and offsets -> onSelectionComplete fires
3.  SelectionPopover appears near selection
4.  User clicks "Ask AI"
5.  invoke create_thread { document_id, page_number, anchor_start, anchor_end, anchor_quote }
6.  Thread row created -> Thread returned
7.  Frontend uses DOM Range or CSS Custom Highlights to permanently mark the selected text
8.  ThreadPanel opens with ThreadHeader showing anchor_quote
9.  User types message -> presses Enter
10. invoke send_message { thread_id, content }
11. Rust persists user message
12. Rust assembles prompt context (KB + text_cache window + summary + recent messages)
13. Rust opens streaming POST to AI provider
14. Tokens arrive -> emit ai://token { thread_id, token } per token
15. Svelte subscription -> tokens appended to StreamingBubble
16. Stream ends -> emit ai://done { thread_id }
17. Rust persists full assistant message
18. Rust spawns async summarization job (tokio::spawn, non-blocking)
      complete_once() -> updates thread.summary
      complete_once() -> upserts kb_entry with concept label
19. Frontend replaces StreamingBubble with rendered MessageBubble
20. ThreadHeader "What we established" updates if summary changed
```

---

## 19. Build Order (MVP)

1. PDF render + Text Layer Alignment Backend — Backend corrects PDF text matrices, PDF.js renders visually aligned text layers. Highest-risk step. Validate first.

2. Text layer quality detection + OCR waterfall — assessTextLayerQuality(), mdls Spotlight query, Vision OCR Swift helper. Test on a known scanned PDF.

3. DB schema + rusqlite — confirm updated schema creates cleanly, run a test command.

4. Thread anchor cycle (no AI) — native HTML selection -> create_thread -> persist -> reload -> confirm highlight redraws and thread loads.

5. Svelte 5 Runes stores + typed IPC layer — tauri.ts typed wrappers, state management.

6. Thread panel static — ThreadHeader, MessageList, ThreadInput. No streaming yet.

7. AI client + streaming — wire send_message, validate token events at frontend,
   StreamingBubble -> MessageBubble transition.

8. KaTeX rendering — validate with a math-heavy response.

9. Summarization job — validate thread.summary updates after 4+ messages, KB entries
   created, ThreadHeader "What we established" section visible.

10. Project management UI — ProjectSidebar, DocumentList, ProjectOnboarding dialogue.

11. Settings panel — provider selection, API key inputs, model selection.

12. Polish — keyboard shortcuts, storage budget display, dark mode, zoom controls,
    last_read_page persistence, collapsed thread marker hover tooltips.

---

## 20. Known Open Questions and Deferred Decisions

Vision OCR integration: Swift subprocess vs objc2 bindings. Subprocess is simpler
to implement and debug. objc2 is faster (no process spawn overhead). Recommend
subprocess for v1, migrate if performance becomes an issue.

Page offset tracking: text_cache is stored as a flat string. For accurate page
windowing in prompt assembly, page_offsets (byte start of each page) should be stored
as a JSON array in documents. Currently using naive 3000-char/page approximation.
Improve before v1 ships.

Cross-document context: architecture supports it (project-scoped KB) but prompt
assembly only windows a single document's text. Cross-document injection is v2.

Semantic KB retrieval: v1 uses keyword/recency match. v2 should use vector embeddings
(fastembed or local model) for semantic retrieval.

Thread marker y-positioning: if a highlight is off-screen (user has scrolled past
it), the collapsed marker should still be visible. Consider a fixed right-margin
column as a fallback for off-screen highlights.

Multi-page selections: current architecture assumes a selection fits within one page.
Selections spanning a page break are not handled. Acceptable for v1.

Diff-awareness: fuzzy re-anchoring handles minor text changes if an updated paper
version is loaded. Major restructuring will break anchors. Out of scope for v1.

Export: thread or project export as markdown/PDF. Natural v2 feature.

Project-level chat: a thread not anchored to any passage, for synthesis questions
across a project. Schema supports it via a virtual document concept. Deferred.

---

## 21. Things Explicitly Decided Against

| Feature | Reason |
|---|---|
| Graph/link views | Obsidian territory, scope creep, contradicts minimal aesthetic |
| Flashcards / SRS | Different product, dilutes focus |
| Heavyweight note editor | Threads are conversational, not documents |
| Copying PDFs into app data | Storage explosion, reference path only |
| Caching assembled AI context | Storage explosion, assemble at query time |
| Accounts / cloud sync | Local-first, no friction on first run, not in v1 |
| Spatial Bounding Box Selection | Too rigid; unable to select partial sentences cleanly. Replaced with dynamic PDF realignment |
| Tesseract for OCR | ~20MB binary bloat, macOS Vision is higher quality and free |
| Tags on documents or threads | KB provides cross-cutting organization, manual tags duplicate effort |
| Ads or usage metering | Antithetical to the product's value proposition |

---

## 22. Critical Technical Gotchas (Virtualization)

During the implementation of the lazy-loaded, memory-efficient PDF rendering pipeline, two critical architectural bugs were uncovered and solved. If rebuilding or refactoring this system, do NOT revert to standard patterns:

### 1. WebKit/Safari IntersectionObserver Flexbox Bug
We initially used Svelte's `use:action` to bind an `IntersectionObserver` to the empty page placeholders in the scrolling flex container. Because Svelte 5 mounts DOM elements natively in microtasks *before* WebKit finalizes its flex layout engine calculations, the Observer sampled the placeholders as having a dimension of `0x0` pixels. Since the elements never actively mutated their size afterward, the observer permanently cached them as "out-of-bounds" and failed to fire.
**Resolution**: `IntersectionObserver` was completely gutted from the PDF Viewer and replaced with **Deterministic Mathematical Virtualization**. The scroll container binds an `onscroll` listener that calculates exact top/bottom geometry `(y = scrollTop)` against the known `defaultPageHeight`, mathematically executing `.rendered = true` with zero reliance on browser compositor APIs.

### 2. Svelte 5 DOM Reference Annihilation (innerHTML Footgun)
In legacy code, `container.innerHTML = ''` was used to clear the PDF view on document change. Because Svelte 5 uses actual HTML comment nodes (`<!--[-->` and `<!--]-->`) invisibly injected into the DOM to anchor `#each` loop lists, manually clearing `innerHTML` silently deletes Svelte's internal layout anchors. 
When the backend later provided the new array of pages to render, Svelte physically could not find its structural mounting points, silently crashing the list rendering while maintaining positive application state. `querySelector` subsequently returned null.
**Resolution**: Never manually mutate or clear the `innerHTML` of any container managed by a Svelte `#each` component. State clears must be done strictly via array assignments (`pages = []`).

---

## 23. The "Apple Live Text" Alignment Safari

Achieving Apple-grade sub-pixel perfect text highlighting in a WebKit Webview (Svelte + `pdf.js`) involved a grueling journey through mathematical transformations and browser typography behaviors.

### Iteration 1: The Native OCR Fallacy
Initial tests demonstrated that `pdf.js` would occasionally drift horizontally when applying selections over scanned or older digital PDFs. Our first attempt bypassed `pdf.js` by orchestrating a heavy backend Swift OCR (`Vision` Framework) sweep. We mistakenly processed the entire document natively, blocking application load, and incorrectly synced the layout against the PDF `.mediaBox` instead of the `.cropBox`.

### Iteration 2: Apple Preview Hybrid Architecture (Dual-Layer)
We realized Apple Preview itself does not use Optical OCR when possible—it pulls exact geometric arrays from the PDF definitions! We rewrote `ocr.swift` to natively query `PDFKit.selectionsByLine()` for perfectly flawless coordinates (sub-1ms execution), returning normalized vector locations for Svelte, and falling back to Neural Vision OCR only if the page was genuinely a flat image.

### Iteration 3: SVG Sub-pixel Exactitude (Bypassing HTML `<span>` Physics)
Despite returning flawless mathematical coordinates, creating standard HTML `<span>` nodes and explicitly positioning them via `top/left/width/height` still produced "shaky" layout discrepancies. **Browsers strictly enforce CSS Typography Layout Algorithms** (kerning, line-height padding, letter-spacing defaults), physically pushing the underlying text out-of-sync horizontally with the bounding boxes.
**The Solution:** We migrated Svelte's DOM injection to a pure Vector layer (`<svg viewBox...>`). By dynamically injecting mathematical `<text>` nodes governed by `textLength="xx"` alongside the explicit override `lengthAdjust="spacingAndGlyphs"`, we forced WebKit to mechanically squash or stretch every letter to geometrically occupy the Apple AppKit boundaries. Horizontal typographic drift is mathematically zeroed out.

---

End of CONTEXT.md
Updated after: canvas selection architecture, SVG sub-pixel highlighting, mathematical virtualization, and dual-layer native AppKit/Vision fallback routing were established.
