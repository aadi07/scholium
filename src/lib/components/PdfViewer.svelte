<script lang="ts">
  import * as pdfjsLib from 'pdfjs-dist';
  import 'pdfjs-dist/build/pdf.worker.mjs';
  import { readFile } from '@tauri-apps/plugin-fs';
  import { onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { TextSelection, Thread, OcrBlock } from '$lib/types';

  interface Props {
    filePath: string | null;
    threads?: Thread[];
    onSelection?: (selection: TextSelection) => void;
    onPageCount?: (count: number) => void;
    onActivePage?: (pageNum: number) => void;
  }

  let { filePath, threads = [], onSelection, onPageCount, onActivePage }: Props = $props();

  let container: HTMLDivElement;
  let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;

  let loading = $state(false);
  let rendering = $state(false);
  let renderedPages = $state(0);
  let totalPages = $state(0);
  let error = $state<string | null>(null);
  let scale = $state(1.5);

  let currentLoadId = 0;
  // [OCR DISABLED FOR V1 - page-level AI mode active]
  // let ocrCache = new Map<number, OcrBlock[]>();
  // let ocrStatus = $state<string | null>(null);
  let ocrCache = new Map<number, OcrBlock[]>(); // kept so cleanupPage refs compile
  let activePage = $state(1);

  let pages = $state<{ pageNum: number; visible: boolean; rendered: boolean }[]>([]);
  let defaultPageWidth = $state(800);
  let defaultPageHeight = $state(1000);
  const activePages = new Map<number, pdfjsLib.PDFPageProxy>();
  let scrollParent: HTMLDivElement | undefined = $state();

  function checkVisiblePages() {
    if (!scrollParent || pages.length === 0) return;
    const y = scrollParent.scrollTop;
    const viewportHeight = scrollParent.clientHeight || window.innerHeight;
    const MARGIN = 1500;
    const viewTop = y - MARGIN;
    const viewBottom = y + viewportHeight + MARGIN;

    const containerTopOffset = 24;
    const itemHeight = defaultPageHeight + 12;

    // Determine which page is centered in the viewport (for AI context)
    const viewCenter = y + viewportHeight / 2;
    let closestPage = 1;
    let closestDist = Infinity;

    for (const p of pages) {
      const pTop = containerTopOffset + ((p.pageNum - 1) * itemHeight);
      const pBottom = pTop + defaultPageHeight;
      const pCenter = (pTop + pBottom) / 2;
      const dist = Math.abs(pCenter - viewCenter);
      if (dist < closestDist) {
        closestDist = dist;
        closestPage = p.pageNum;
      }

      const isIntersecting = pBottom >= viewTop && pTop <= viewBottom;

      if (isIntersecting) {
        if (!p.visible) {
          p.visible = true;
          if (!p.rendered) {
            p.rendered = true;
            renderPage(p.pageNum, currentLoadId).catch((err: Error) => {
              error = `Failed to render page ${p.pageNum}: ${err.message}`;
            });
          }
        }
      } else {
        if (p.visible) {
          p.visible = false;
          if (p.rendered) {
            p.rendered = false;
            cleanupPage(p.pageNum);
          }
        }
      }
    }

    if (closestPage !== activePage) {
      activePage = closestPage;
      onActivePage?.(closestPage);
    }
  }

  // Guards against reopening the exact same file unnecessarily.
  let loadedPath: string | null = null;
  let openingPath: string | null = null;

  $effect(() => {
    const path = filePath;

    if (!path) {
      loadedPath = null;
      openingPath = null;
      destroyCurrentDocument();
      return;
    }

    // Already fully loaded for this exact path.
    if (loadedPath === path && pdfDoc) {
      return;
    }

    // Already in progress for this exact path.
    if (openingPath === path) {
      return;
    }

    void loadPdf(path);
  });

  $effect(() => {
    // Re-apply highlights after all pages render, or if threads completely change.
    // Ensure we are not currently rendering.
    if (threads && !rendering && pdfDoc && container) {
      applyHighlights();
    }
  });



  onDestroy(() => {
    if ((window as any).CSS && (window as any).CSS.highlights) {
      (window as any).CSS.highlights.delete('scholium-thread');
    }
    destroyCurrentDocument();
  });

  function destroyCurrentDocument() {
    currentLoadId++;

    const doc = pdfDoc;
    pdfDoc = null;

    pages = [];
    activePages.forEach(p => p.cleanup());
    activePages.clear();

    loading = false;
    rendering = false;
    renderedPages = 0;
    totalPages = 0;
    error = null;

    if (doc) {
      void doc.destroy().catch((e) => {
        console.warn('Failed to destroy PDF document:', e);
      });
    }
  }

  function waitForPaint(): Promise<void> {
    return new Promise((resolve) => {
      requestAnimationFrame(() => {
        setTimeout(resolve, 0);
      });
    });
  }

  async function loadPdf(path: string) {
    const myLoadId = ++currentLoadId;
    const oldDoc = pdfDoc;

    openingPath = path;
    pdfDoc = null;
    loadedPath = null;
    error = null;
    loading = true;
    rendering = false;
    renderedPages = 0;
    totalPages = 0;

    pages = [];
    ocrCache.clear();
    // ocrStatus = null;

    try {
      console.time('[pdf] total open');

      if (oldDoc) {
        await oldDoc.destroy().catch((e) => {
          console.warn('Failed to destroy previous PDF document:', e);
        });
      }

      console.time('[pdf] readFile');
      const bytes = await readFile(path);
      console.timeEnd('[pdf] readFile');

      if (currentLoadId !== myLoadId) return;

      console.log('[pdf] bytes length:', bytes.length);

      const loadingTask = pdfjsLib.getDocument({
        data: bytes,
        disableRange: true,
        disableStream: true,
        disableAutoFetch: true,
        useWasm: false,
        isImageDecoderSupported: false,
        isOffscreenCanvasSupported: false,
        enableXfa: false,
        stopAtErrors: true
      });

      loadingTask.onProgress = (p: { loaded: number; total: number }) => {
        console.log('[pdf] onProgress:', p);
      };

      const doc = await loadingTask.promise;
      if (currentLoadId !== myLoadId) {
        await doc.destroy().catch(() => {});
        return;
      }

      console.timeEnd('[pdf] total open');
      console.log('[pdf] opened successfully, pages:', doc.numPages);

      pdfDoc = doc;
      loadedPath = path;
      openingPath = null;
      totalPages = doc.numPages;
      onPageCount?.(doc.numPages);

      const initPage = await doc.getPage(1);
      const initViewport = initPage.getViewport({ scale });
      defaultPageWidth = initViewport.width;
      defaultPageHeight = initViewport.height;
      initPage.cleanup();

      pages = Array.from({ length: doc.numPages }, (_, i) => ({
        pageNum: i + 1,
        visible: false,
        rendered: false
      }));

      loading = false;
      // Fire onActivePage(1) immediately so the parent can pre-fetch page 1 text
      // before the user sends a message. checkVisiblePages only fires when the
      // page changes, so this explicit call handles the initial load case.
      activePage = 0; // reset so checkVisiblePages triggers on first run
      onActivePage?.(1);
      setTimeout(() => checkVisiblePages(), 50);
    } catch (e) {
      if (currentLoadId === myLoadId) {
        openingPath = null;
        loadedPath = null;
        error = `Failed to load PDF: ${e}`;
        loading = false;
        rendering = false;
        console.error('[pdf] load failed:', e);
      }
    }
  }

  async function renderPage(
    pageNum: number,
    loadId: number
  ) {
    if (!pdfDoc) return;
    const doc = pdfDoc;
    const pageWrapper = container?.querySelector(`[data-page-number="${pageNum}"]`) as HTMLElement;
    if (!pageWrapper) {
       throw new Error(`CRITICAL: Svelte did not mount DOM for page ${pageNum} in time.`);
    }

    // ── [HYBRID OCR ENGINE COMMENTED OUT FOR V1 — page-level AI mode] ──────────
    // When re-enabling: uncomment the block below.
    // This uses a dual-layer approach: PDFKit native selectionsByLine() first,
    // Vision OCR fallback for scanned-only pages. Results go into ocrCache.
    // The SVG overlay below (also commented) then renders them as selectable text.
    //
    // if (!ocrCache.has(pageNum - 1) && filePath) {
    //   ocrStatus = `Scanning page ${pageNum}...`;
    //   const t0 = performance.now();
    //   try {
    //     const jsonStr = await invoke<string>('extract_vision_ocr', {
    //       filePath: filePath,
    //       pageNumber: pageNum - 1
    //     });
    //     if (currentLoadId !== loadId) return;
    //     const blocks = JSON.parse(jsonStr);
    //     ocrCache.set(pageNum - 1, blocks);
    //     if (blocks.length > 0) {
    //       const sourceType = blocks[0].source === 'native' ? 'AppKit Native' : 'Vision Optical';
    //       ocrStatus = `Engine: Page ${pageNum} [${sourceType}] (${Math.round(performance.now() - t0)}ms)`;
    //     } else {
    //       ocrStatus = `Engine: Page ${pageNum} yielded 0 blocks.`;
    //     }
    //   } catch (e) {
    //     console.warn(`Vision OCR failed on page ${pageNum}:`, e);
    //     ocrCache.set(pageNum - 1, []);
    //     ocrStatus = `Engine Error: ${e}`;
    //   }
    // }
    // ─────────────────────────────────────────────────────────────────────────────

    const page = await doc.getPage(pageNum);
    if (currentLoadId !== loadId) return;

    activePages.set(pageNum, page);

    const dpr = window.devicePixelRatio || 1;
    const viewport = page.getViewport({ scale });

    const cssWidth = viewport.width;
    const cssHeight = viewport.height;
    
    pageWrapper.style.width = `${cssWidth}px`;
    pageWrapper.style.minHeight = `${cssHeight}px`;

    const pixelWidth = Math.floor(cssWidth * dpr);
    const pixelHeight = Math.floor(cssHeight * dpr);

    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');
    if (!context) return;

    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;
    canvas.style.display = 'block';

    const textLayerDiv = document.createElement('div');
    textLayerDiv.className = 'textLayer';

    pageWrapper.innerHTML = ''; 
    pageWrapper.appendChild(canvas);
    pageWrapper.appendChild(textLayerDiv);

    const transform = dpr !== 1 ? ([dpr, 0, 0, dpr, 0, 0] as [number, number, number, number, number, number]) : undefined;

    await page.render({
      canvasContext: context,
      canvas,
      viewport,
      transform
    }).promise;

    if (currentLoadId !== loadId) return;

    // ── [SVG VECTOR OVERLAY COMMENTED OUT FOR V1 — restore for sub-pixel selection] ──
    // Architecture: SVG <text> nodes with textLength + lengthAdjust="spacingAndGlyphs"
    // force WebKit to mechanically stretch glyphs to match AppKit bounding boxes exactly,
    // bypassing browser CSS typography layout (kerning, line-height, letter-spacing).
    // Re-enable together with the OCR block above once page-level AI is stable.
    //
    // const pageOcr = ocrCache.get(pageNum - 1) || [];
    // if (pageOcr.length > 0) {
    //   const svgNs = 'http://www.w3.org/2000/svg';
    //   const svg = document.createElementNS(svgNs, 'svg');
    //   svg.classList.add('synthetic-vision');
    //   svg.setAttribute('viewBox', '0 0 100 100');
    //   svg.setAttribute('preserveAspectRatio', 'none');
    //   svg.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;z-index:1;';
    //   for (const block of pageOcr) {
    //     const text = document.createElementNS(svgNs, 'text');
    //     text.textContent = block.text;
    //     text.setAttribute('x', String(block.x * 100));
    //     text.setAttribute('y', String((block.y + block.height * 0.85) * 100));
    //     text.setAttribute('textLength', String(block.width * 100));
    //     text.setAttribute('lengthAdjust', 'spacingAndGlyphs');
    //     text.setAttribute('font-size', String(block.height * 100));
    //     text.setAttribute('fill', 'rgba(255,0,0,0.4)'); // diagnostic red
    //     text.style.cssText = 'font-family:sans-serif;white-space:pre;cursor:text;user-select:text;';
    //     svg.appendChild(text);
    //   }
    //   pageWrapper.appendChild(svg);
    // } else { ... native pdf.js layer ... }
    // ─────────────────────────────────────────────────────────────────────────────

    // V1: always use the native pdf.js text layer
    try {
      const textStream = page.streamTextContent();
      const textLayer = new pdfjsLib.TextLayer({
        textContentSource: textStream,
        container: textLayerDiv,
        viewport
      });
      await textLayer.render();
    } catch (e) {
      console.warn(`Text layer failed on page ${pageNum}:`, e);
    }

    applyHighlights();
  }

  function cleanupPage(pageNum: number) {
    const page = activePages.get(pageNum);
    if (page) {
      page.cleanup();
      activePages.delete(pageNum);
    }
    const pageWrapper = container?.querySelector(`[data-page-number="${pageNum}"]`) as HTMLElement;
    if (pageWrapper) {
      pageWrapper.innerHTML = '';
      pageWrapper.style.width = `${defaultPageWidth}px`;
      pageWrapper.style.minHeight = `${defaultPageHeight}px`;
    }
    applyHighlights();
  }

  function getTextNodeOffset(root: Element, targetNode: Node, targetOffset: number): number {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let currentOffset = 0;
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (node === targetNode) {
        return currentOffset + targetOffset;
      }
      currentOffset += node.textContent?.length || 0;
    }
    return currentOffset;
  }

  function findTextNodeByOffset(root: Element, targetOffset: number): { node: Node, offset: number } | null {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let currentOffset = 0;
    while (walker.nextNode()) {
      const node = walker.currentNode;
      const len = node.textContent?.length || 0;
      if (currentOffset + len >= targetOffset) {
        return { node, offset: targetOffset - currentOffset };
      }
      currentOffset += len;
    }
    return null;
  }

  function applyHighlights() {
    const CSS = (window as any).CSS;
    const Highlight = (window as any).Highlight;
    if (!CSS || !CSS.highlights || !Highlight) return;

    const highlightRanges: Range[] = [];

    const pages = container?.querySelectorAll('.pdf-page') || [];
    for (const pageEl of Array.from(pages)) {
      const pageNum = parseInt((pageEl as HTMLElement).dataset.pageNumber ?? '1', 10) - 1;
      const textLayerEl = pageEl.querySelector('.textLayer, .synthetic-vision') as Element | null;
      if (!textLayerEl) continue;

      const pageThreads = threads.filter(t => t.page_number === pageNum);
      for (const t of pageThreads) {
        if (t.anchor_start == null || t.anchor_end == null) continue;

        const startRes = findTextNodeByOffset(textLayerEl, t.anchor_start);
        const endRes = findTextNodeByOffset(textLayerEl, t.anchor_end);

        if (startRes && endRes) {
          try {
            const range = new Range();
            range.setStart(startRes.node, startRes.offset);
            range.setEnd(endRes.node, endRes.offset);
            highlightRanges.push(range);
          } catch(e) {
            console.warn("Failed to create range for thread", t.id, e);
          }
        }
      }
    }

    if (highlightRanges.length > 0) {
      const highlight = new Highlight(...highlightRanges);
      CSS.highlights.set('scholium-thread', highlight);
    } else {
      CSS.highlights.delete('scholium-thread');
    }
  }

  function handleMouseUp() {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.toString().trim()) return;

    const selectedText = sel.toString().trim();
    const range = sel.getRangeAt(0);

    let node: Node | null = range.startContainer;
    let pageEl: HTMLElement | null = null;
    let textLayerEl: Element | null = null;

    while (node && node !== container) {
      if (node instanceof Element && (node.classList.contains('textLayer') || node.classList.contains('synthetic-vision'))) {
        textLayerEl = node;
      }
      if (node instanceof HTMLElement && node.classList.contains('pdf-page')) {
        pageEl = node;
        break;
      }
      node = node.parentNode;
    }

    if (!pageEl || !textLayerEl) return;

    const pageNumber = parseInt(pageEl.dataset.pageNumber ?? '1', 10) - 1;
    const anchorStart = getTextNodeOffset(textLayerEl, range.startContainer, range.startOffset);
    const anchorEnd = getTextNodeOffset(textLayerEl, range.endContainer, range.endOffset);

    // Make sure anchorEnd >= anchorStart handles backward selections cleanly
    const safeStart = Math.min(anchorStart, anchorEnd);
    const safeEnd = Math.max(anchorStart, anchorEnd);

    onSelection?.({
      text: selectedText,
      pageNumber,
      anchorStart: safeStart,
      anchorEnd: safeEnd
    });
  }

  async function rerenderCurrentDocument() {
    if (!pdfDoc) return;

    const myLoadId = ++currentLoadId;
    const doc = pdfDoc;

    error = null;
    rendering = false;
    renderedPages = 0;

    pages = [];
    activePages.forEach(p => p.cleanup());
    activePages.clear();

    await waitForPaint();
    if (currentLoadId !== myLoadId) return;

    const initPage = await doc.getPage(1);
    const initViewport = initPage.getViewport({ scale });
    defaultPageWidth = initViewport.width;
    defaultPageHeight = initViewport.height;
    initPage.cleanup();

    pages = Array.from({ length: doc.numPages }, (_, i) => ({
      pageNum: i + 1,
      visible: false,
      rendered: false
    }));

    setTimeout(() => checkVisiblePages(), 50);
  }

  export function zoomIn() {
    scale = Math.min(scale + 0.25, 4);
    void rerenderCurrentDocument();
  }

  export function zoomOut() {
    scale = Math.max(scale - 0.25, 0.5);
    void rerenderCurrentDocument();
  }

  /**
   * Extract the full plain text of a given page directly from the in-memory
   * pdf.js document object. This is DOM-independent and always up-to-date.
   * Returns empty string if the doc isn't loaded yet.
   */
  /**
   * Render a page to an offscreen canvas and return it as a base64 PNG data URL.
   * Used to send the page image to a vision-language model.
   */
  export async function getPageImage(pageNum: number, renderScale = 2): Promise<string> {
    if (!pdfDoc) return '';
    try {
      const page = await pdfDoc.getPage(pageNum);
      const viewport = page.getViewport({ scale: renderScale });

      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);

      const ctx = canvas.getContext('2d');
      if (!ctx) { page.cleanup(); return ''; }

      await page.render({ canvasContext: ctx, canvas, viewport }).promise;
      page.cleanup();

      return canvas.toDataURL('image/png');
    } catch (e) {
      console.warn('[getPageImage] failed:', e);
      return '';
    }
  }

  export async function getPageText(pageNum: number): Promise<string> {
    if (!pdfDoc) return '';
    try {
      const page = await pdfDoc.getPage(pageNum);
      const content = await page.getTextContent();
      page.cleanup();

      if (content.items.length === 0) return '';

      // Reconstruct reading order using spatial coordinates from the PDF transform.
      // Each item has transform[4]=x, transform[5]=y (PDF space, y increases upward).
      // We group items by Y position (within half a line-height tolerance), sort groups
      // top-to-bottom, and sort items within each group left-to-right. This preserves
      // line structure for prose and keeps math symbols on the same line as their operators.
      type LineItem = { x: number; str: string };
      const groups: { y: number; items: LineItem[] }[] = [];

      for (const raw of content.items) {
        if (!('str' in raw) || !raw.str) continue;
        const item = raw as any;
        const x: number = item.transform[4];
        const y: number = item.transform[5];
        const height: number = item.height || 10;
        const tolerance = Math.max(height * 0.5, 3);

        const group = groups.find(g => Math.abs(g.y - y) <= tolerance);
        if (group) {
          group.items.push({ x, str: item.str });
        } else {
          groups.push({ y, items: [{ x, str: item.str }] });
        }
      }

      // Sort groups top-to-bottom (highest Y value = top of page in PDF coords)
      groups.sort((a, b) => b.y - a.y);

      const lines = groups.map(g => {
        g.items.sort((a, b) => a.x - b.x);
        return g.items.map(i => i.str).join('');
      });

      return lines.filter(l => l.trim()).join('\n');
    } catch (e) {
      console.warn('[getPageText] failed:', e);
      return '';
    }
  }
</script>

<div class="relative flex flex-col h-full w-full overflow-hidden bg-[#1a1a1a]">
  {#if loading}
    <div class="absolute inset-0 flex items-center justify-center z-10 bg-[#1a1a1a]/80">
      <div class="flex flex-col items-center gap-3">
        <div class="w-6 h-6 border-2 border-indigo-400 border-t-transparent rounded-full animate-spin"></div>
        <span class="text-sm text-zinc-400">Opening PDF…</span>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="absolute inset-0 flex items-center justify-center z-20">
      <p class="text-sm text-red-500 max-w-sm text-center bg-black/80 px-4 py-2 rounded font-mono">{error}</p>
    </div>
  {/if}

  {#if !loading && !error && filePath}
    <div class="absolute bottom-4 right-4 z-10 flex items-center gap-1 bg-[#222] border border-white/10 rounded-lg px-1 py-0.5">
      <button
        onclick={zoomOut}
        class="w-7 h-7 flex items-center justify-center rounded text-zinc-400 hover:text-white hover:bg-white/10 transition-colors text-lg leading-none"
        title="Zoom out"
      >−</button>
      <span class="text-xs text-zinc-500 w-10 text-center tabular-nums">{Math.round(scale * 100)}%</span>
      <button
        onclick={zoomIn}
        class="w-7 h-7 flex items-center justify-center rounded text-zinc-400 hover:text-white hover:bg-white/10 transition-colors text-lg leading-none"
        title="Zoom in"
      >+</button>
    </div>
  {/if}

  {#if rendering && !loading && !error}
    <div class="absolute top-4 right-4 z-10 bg-[#222]/95 border border-white/10 rounded-lg px-3 py-2">
      <div class="text-xs text-zinc-300">Rendering pages…</div>
      <div class="text-[11px] text-zinc-500 tabular-nums">{renderedPages} / {totalPages}</div>
    </div>
  {/if}

  <!-- [OCR STATUS BANNER DISABLED FOR V1] -->
  <!-- {#if ocrStatus} ... {/if} -->

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={scrollParent}
    class="flex-1 overflow-y-auto overflow-x-auto"
    onmouseup={handleMouseUp}
    onscroll={checkVisiblePages}
  >
    <div
      bind:this={container}
      class="flex flex-col items-center py-6 px-4 min-h-full gap-[12px]"
    >
      {#each pages as p (p.pageNum)}
        <div
          data-page-number={p.pageNum}
          class="pdf-page bg-white shadow-sm flex-shrink-0 relative"
          style="width: {defaultPageWidth}px; min-height: {defaultPageHeight}px;"
        ></div>
      {/each}
    </div>
  </div>
</div>