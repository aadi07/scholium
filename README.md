# Scholium

A macOS PDF reader built around the idea that your conversations with an AI about what you're reading should live *inside* the document, attached to the exact passages that prompted them.

The problem it's solving: you're stuck on a dense paper, you paste a passage into ChatGPT, get a great explanation, go back to reading — and two weeks later you hit the same wall and have no idea you already worked through it. Scholium fixes this by anchoring every conversation to the text selection that started it, persisting it, and surfacing it automatically when you return to that passage.

## What's working right now

- PDF rendering with lazy page virtualization (only renders what's on screen)
- Page-level AI chat using **gemma4:26b** via Ollama — the model sees a rendered image of the current page, so figures, equations, and layout all come through
- Full conversation history persists per document across sessions (SQLite)
- KaTeX math rendering in AI responses — handles `$...$`, `$$...$$`, `\(...\)`, `\[...\]`
- LaTeX input toolbar for composing math in your questions
- Text selection with character-level anchoring (groundwork for thread anchoring)

## What's being built next

The core product mechanic: selecting a passage → "Ask AI" popover → conversation anchored to that highlight, visible in the margin, restored on every future visit.

## Stack

- **Tauri** (Rust backend, native macOS webview)
- **SvelteKit + TypeScript + Vite**
- **TailwindCSS**
- **pdf.js** for rendering
- **KaTeX** for math
- **SQLite** via rusqlite (shared Arc/Mutex connection, WAL mode)
- **Ollama** for local AI (no API key needed, fully offline)

## Running it

You'll need [Ollama](https://ollama.com) running locally with gemma4:26b pulled:

```bash
ollama pull gemma4:26b
```

Then:

```bash
npm run tauri dev
```

Rust, Node, and the Tauri CLI are prerequisites — see [Tauri's setup guide](https://tauri.app/start/prerequisites/) if you haven't done this before.

## Dev notes

- DB lives at `~/Library/Application Support/scholium/scholium.db` — not in the repo
- Don't put the DB inside `src-tauri/` or Tauri's file watcher will detect SQLite's WAL writes and restart the app in an infinite loop
- See `CONTEXT.md` for full architecture decisions and product reasoning
