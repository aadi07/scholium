<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import PdfViewer from '$lib/components/PdfViewer.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { app } from '$lib/stores/app.svelte';
  import type { TextSelection, Thread } from '$lib/types';

  // ─── File open ─────────────────────────────────────────────────────────────

  async function openFile() {
    const path = await open({
      title: 'Open PDF',
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      multiple: false,
    });

    if (!path || typeof path !== 'string') return;

    const fileName = path.split('/').pop() ?? path;

    app.activeDocument = {
      id: crypto.randomUUID(),
      project_id: '',
      file_path: path,
      title: fileName.replace(/\.pdf$/i, ''),
      created_at: Date.now(),
    };
    app.activeSelection = null;
    app.activeThread = null;
    activePage = 1;
    pageContextText = '';
    messages = [];
    streamingBuffer = '';
  }

  // ─── Page-level AI state ────────────────────────────────────────────────────

  let activePage = $state(1);
  let pageContextText = $state('');
  let aiPanelOpen = $state(false);  // auto-opens when doc is loaded

  function handleActivePage(pageNum: number) {
    activePage = pageNum;
    // Reset context — will be grabbed fresh when user sends a message
    pageContextText = '';
  }

  function handlePageCount(count: number) {
    if (app.activeDocument) {
      app.activeDocument = { ...app.activeDocument, page_count: count };
    }
    aiPanelOpen = true;
  }

  /** Extract visible text from the currently active pdf-page DOM node */
  function extractPageText(pageNum: number): string {
    const pageEl = document.querySelector(`[data-page-number="${pageNum}"]`);
    if (!pageEl) return '';
    // Grab text from the native pdf.js textLayer spans
    const textLayer = pageEl.querySelector('.textLayer');
    if (!textLayer) return '';
    return (textLayer as HTMLElement).innerText ?? textLayer.textContent ?? '';
  }

  // ─── Selection handling (preserved for future thread anchoring) ─────────────

  function handleSelection(selection: TextSelection) {
    app.activeSelection = selection;
  }

  // ─── Thread messaging state ─────────────────────────────────────────────────

  let messages = $state<{ id: string; thread_id: string; role: string; content: string; created_at: number; }[]>([]);
  let outgoing = $state('');
  let streamingBuffer = $state('');
  let streaming = $state(false);
  let tokenUnlisten: (() => void) | null = null;
  let doneUnlisten: (() => void) | null = null;
  let errorUnlisten: (() => void) | null = null;
  let messagesEnd: HTMLDivElement;

  // Page-level thread ID — one ephemeral conversation per session per document
  // (not persisted across restarts for V1, just in-memory)
  let pageThreadId = $state<string>(crypto.randomUUID());

  $effect(() => {
    // Reset conversation when document changes
    if (app.activeDocument) {
      pageThreadId = crypto.randomUUID();
      messages = [];
      streamingBuffer = '';
      streaming = false;
    }
  });

  $effect(() => {
    // Auto-scroll to bottom when new content arrives
    if (messagesEnd && (messages.length > 0 || streamingBuffer)) {
      messagesEnd.scrollIntoView({ behavior: 'smooth' });
    }
  });

  onMount(async () => {
    const tokenListener = await listen('ai://token', (e) => {
      const payload = e.payload as any;
      if (payload.thread_id !== pageThreadId) return;
      streamingBuffer += payload.token;
    });

    const doneListener = await listen('ai://done', (e) => {
      const payload = e.payload as any;
      if (payload.thread_id !== pageThreadId) return;
      // Commit streamed content as a full message
      if (streamingBuffer.trim()) {
        messages = [...messages, {
          id: crypto.randomUUID(),
          thread_id: pageThreadId,
          role: 'assistant',
          content: streamingBuffer,
          created_at: Date.now()
        }];
      }
      streamingBuffer = '';
      streaming = false;
    });

    const errorListener = await listen('ai://error', (e) => {
      const payload = e.payload as any;
      if (payload.thread_id !== pageThreadId) return;
      streaming = false;
      streamingBuffer = '';
      messages = [...messages, {
        id: crypto.randomUUID(),
        thread_id: pageThreadId,
        role: 'assistant',
        content: `⚠️ ${payload.error}`,
        created_at: Date.now()
      }];
    });

    tokenUnlisten = tokenListener;
    doneUnlisten = doneListener;
    errorUnlisten = errorListener;
  });

  onDestroy(() => {
    if (tokenUnlisten) void tokenUnlisten();
    if (doneUnlisten) void doneUnlisten();
    if (errorUnlisten) void errorUnlisten();
  });

  async function sendMessage() {
    if (!app.activeDocument || !outgoing.trim() || streaming) return;

    const userText = outgoing.trim();
    outgoing = '';

    // Extract live page text right before sending
    const ctx = extractPageText(activePage);

    // Optimistically add user message to UI
    messages = [...messages, {
      id: crypto.randomUUID(),
      thread_id: pageThreadId,
      role: 'user',
      content: userText,
      created_at: Date.now()
    }];

    streamingBuffer = '';
    streaming = true;

    try {
      await invoke('send_message', {
        thread_id: pageThreadId,
        role: 'user',
        content: userText,
        page_context: ctx,
        page_number: activePage,
      });
    } catch (e) {
      streaming = false;
      console.error('send_message failed:', e);
      messages = [...messages, {
        id: crypto.randomUUID(),
        thread_id: pageThreadId,
        role: 'assistant',
        content: `⚠️ Failed to send: ${e}`,
        created_at: Date.now()
      }];
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'o') {
      e.preventDefault();
      openFile();
    }
    // Cmd+Enter or Ctrl+Enter to send
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      const target = e.target as HTMLElement;
      if (target?.closest('.ai-input-area')) {
        e.preventDefault();
        void sendMessage();
      }
    }
  }

  function formatMessage(content: string) {
    // Very light markdown: bold, code spans, newlines
    return content
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/`([^`]+)`/g, '<code class="font-mono bg-white/10 px-1 rounded text-[11px]">$1</code>')
      .replace(/\n/g, '<br>');
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Root layout -->
<div class="flex h-screen w-screen overflow-hidden bg-[#141414] text-[#e8e6e3]">

  <!-- ── PDF Viewer panel ─────────────────────────────────────────────────── -->
  <div class="flex flex-col flex-1 min-w-0 border-r border-white/[0.06]">

    <!-- Toolbar -->
    <header class="flex items-center gap-3 px-4 h-10 border-b border-white/[0.06] flex-shrink-0">
      <button
        onclick={openFile}
        class="flex items-center gap-1.5 text-xs text-zinc-400 hover:text-zinc-200 transition-colors"
        title="Open PDF (⌘O)"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/>
        </svg>
        Open PDF
      </button>

      {#if app.activeDocument}
        <span class="text-white/20 text-xs">·</span>
        <span class="text-xs text-zinc-300 truncate max-w-xs">
          {app.activeDocument.title}
        </span>
        {#if app.activeDocument.page_count}
          <span class="text-xs text-zinc-600 ml-auto flex-shrink-0">
            {app.activeDocument.page_count} pages
          </span>
        {/if}
      {/if}
    </header>

    <!-- PDF content area -->
    <div class="flex-1 min-h-0">
      {#if app.activeDocument}
        <PdfViewer
          filePath={app.activeDocument.file_path}
          threads={app.threads}
          onSelection={handleSelection}
          onPageCount={handlePageCount}
          onActivePage={handleActivePage}
        />
      {:else}
        <!-- Drop zone / landing state -->
        <div
          class="flex flex-col items-center justify-center h-full gap-5 select-none"
          role="button"
          tabindex="0"
          onclick={openFile}
          onkeydown={(e) => e.key === 'Enter' && openFile()}
        >
          <div class="w-16 h-16 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center">
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#6366f1" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="12" y1="18" x2="12" y2="12"/>
              <line x1="9" y1="15" x2="15" y2="15"/>
            </svg>
          </div>
          <div class="text-center">
            <p class="text-sm text-zinc-300 font-medium">Open a PDF to start reading</p>
            <p class="text-xs text-zinc-600 mt-1">Click here or press <kbd class="font-mono bg-white/[0.06] px-1 py-0.5 rounded text-zinc-400">⌘O</kbd></p>
          </div>
        </div>
      {/if}
    </div>
  </div>

  <!-- ── AI Chat panel (right) ─────────────────────────────────────────────── -->
  <div class="flex flex-col w-[380px] flex-shrink-0 bg-[#161616]">

    <!-- Panel header -->
    <header class="flex items-center justify-between px-4 h-10 border-b border-white/[0.06] flex-shrink-0">
      <div class="flex items-center gap-2">
        <!-- Spark icon -->
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#818cf8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
        </svg>
        <span class="text-xs text-zinc-500 font-medium tracking-wide uppercase">Ask AI</span>
      </div>
      {#if app.activeDocument}
        <span class="text-[11px] text-indigo-400/80 font-mono tabular-nums">
          Page {activePage}
        </span>
      {/if}
    </header>

    {#if app.activeDocument}
      <!-- Current page context chip -->
      <div class="px-3 pt-3">
        <div class="flex items-center gap-1.5 text-[11px] text-zinc-500 bg-white/[0.03] border border-white/[0.06] rounded-lg px-3 py-2">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
          <span>Discussing <strong class="text-zinc-300">{app.activeDocument.title}</strong>, page <strong class="text-zinc-300">{activePage}</strong></span>
        </div>
      </div>

      <!-- Message list -->
      <div class="flex-1 overflow-y-auto px-3 py-3 flex flex-col gap-3 min-h-0">
        {#if messages.length === 0 && !streaming}
          <div class="flex flex-col items-center justify-center h-full gap-3 text-center select-none">
            <div class="w-10 h-10 rounded-xl bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#818cf8" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
              </svg>
            </div>
            <div>
              <p class="text-sm text-zinc-400 font-medium">Ask about this page</p>
              <p class="text-xs text-zinc-600 mt-1">The AI sees the full text of page {activePage}.</p>
            </div>
          </div>
        {/if}

        {#each messages as m (m.id)}
          {#if m.role === 'user'}
            <div class="flex justify-end">
              <div class="max-w-[85%] bg-indigo-600/80 rounded-2xl rounded-tr-sm px-3 py-2">
                <p class="text-sm text-white leading-relaxed">{m.content}</p>
              </div>
            </div>
          {:else}
            <div class="flex justify-start">
              <div class="max-w-[90%] bg-white/[0.04] border border-white/[0.06] rounded-2xl rounded-tl-sm px-3 py-2">
                <!-- svelte-ignore a11y_no-static-element-interactions -->
                <p class="text-sm text-zinc-200 leading-relaxed">{@html formatMessage(m.content)}</p>
              </div>
            </div>
          {/if}
        {/each}

        <!-- Streaming bubble -->
        {#if streaming && streamingBuffer}
          <div class="flex justify-start">
            <div class="max-w-[90%] bg-white/[0.04] border border-white/[0.06] rounded-2xl rounded-tl-sm px-3 py-2">
              <!-- svelte-ignore a11y_no-static-element-interactions -->
              <p class="text-sm text-zinc-200 leading-relaxed">{@html formatMessage(streamingBuffer)}</p>
            </div>
          </div>
        {:else if streaming}
          <!-- Typing indicator -->
          <div class="flex justify-start">
            <div class="bg-white/[0.04] border border-white/[0.06] rounded-2xl rounded-tl-sm px-3 py-2.5 flex items-center gap-1">
              <span class="w-1.5 h-1.5 bg-zinc-500 rounded-full animate-bounce" style="animation-delay:0ms"></span>
              <span class="w-1.5 h-1.5 bg-zinc-500 rounded-full animate-bounce" style="animation-delay:120ms"></span>
              <span class="w-1.5 h-1.5 bg-zinc-500 rounded-full animate-bounce" style="animation-delay:240ms"></span>
            </div>
          </div>
        {/if}

        <div bind:this={messagesEnd}></div>
      </div>

      <!-- Input bar -->
      <div class="ai-input-area px-3 pb-3 flex-shrink-0 border-t border-white/[0.06] pt-3">
        <div class="flex flex-col gap-2">
          <textarea
            bind:value={outgoing}
            rows={3}
            disabled={streaming}
            placeholder="Ask anything about page {activePage}… (⌘↵ to send)"
            class="w-full bg-white/[0.04] border border-white/[0.08] rounded-xl p-3 text-sm text-zinc-200 placeholder-zinc-600 resize-none focus:outline-none focus:border-indigo-500/50 transition-colors disabled:opacity-50"
          ></textarea>
          <button
            onclick={sendMessage}
            disabled={streaming || !outgoing.trim()}
            class="w-full py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 disabled:cursor-not-allowed text-white text-sm font-medium transition-colors flex items-center justify-center gap-2"
          >
            {#if streaming}
              <span class="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin"></span>
              Thinking…
            {:else}
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
              </svg>
              Send
            {/if}
          </button>
        </div>
      </div>

    {:else}
      <!-- No document open -->
      <div class="flex-1 flex items-center justify-center">
        <p class="text-sm text-zinc-700">Open a PDF to begin</p>
      </div>
    {/if}

  </div>

</div>
