<script lang="ts">
  import { Channel } from "@tauri-apps/api/core";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { untrack } from "svelte";
  import "@xterm/xterm/css/xterm.css";
  import type { TerminalEvent } from "./api";
  import { api } from "./api";
  import { toastError } from "./toast.svelte";
  import Icon from "./Icon.svelte";

  // Stays mounted while closed so Claude keeps running in the background;
  // `open` only slides the drawer in and out. When Claude exits the drawer
  // closes, and the next open starts a fresh session.
  // `fill`: take all remaining width (the viewer is collapsed) instead of the
  // drawer's own resizable width.
  let {
    open,
    fill = false,
    onClose,
  }: { open: boolean; fill?: boolean; onClose: () => void } = $props();

  // A session that dies this fast failed to start (e.g. `claude` not on PATH);
  // stay open so its error is readable.
  const QUICK_EXIT_MS = 3000;

  const WIDTH_KEY = "deskwork.terminalWidth";
  const MIN_W = 360;
  const DEFAULT_W = 560;
  const MIN_MAIN = 420; // keep the rest of the app usable

  let width = $state(readWidth());
  let dragging = $state(false);
  let exited = $state(false);
  let host: HTMLDivElement | undefined = $state();

  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let channel: Channel<TerminalEvent> | null = null;
  let startedAt = 0;

  function readWidth(): number {
    try {
      const w = Number(localStorage.getItem(WIDTH_KEY));
      return w >= MIN_W ? w : DEFAULT_W;
    } catch {
      return DEFAULT_W;
    }
  }

  function clampWidth(w: number): number {
    return Math.max(MIN_W, Math.min(w, window.innerWidth - MIN_MAIN));
  }

  function cssVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function theme(): ITheme {
    return {
      background: cssVar("--panel"),
      foreground: cssVar("--text"),
      cursor: cssVar("--accent"),
      cursorAccent: cssVar("--panel"),
      selectionBackground: cssVar("--accent-soft"),
    };
  }

  function fitNow() {
    if (!fit || !host || host.clientWidth === 0) return;
    try {
      fit.fit();
    } catch {
      // xterm throws if measured before its renderer is ready; the next resize retries.
    }
  }

  function init(el: HTMLDivElement) {
    term = new Terminal({
      fontFamily: cssVar("--font-mono") || "Menlo, monospace",
      fontSize: 12.5,
      lineHeight: 1.15,
      cursorBlink: true,
      macOptionIsMeta: true,
      scrollback: 5000,
      theme: theme(),
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);

    term.onData((d) => void api.terminalWrite(d).catch(() => {}));
    term.onResize(({ cols, rows }) => void api.terminalResize(cols, rows).catch(() => {}));
    // ⌘C copies the selection; xterm's selection isn't a DOM selection, so the
    // menu's Copy can't see it. ⌘V arrives as a normal paste event.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type === "keydown" && e.metaKey && e.key === "c" && term?.hasSelection()) {
        void navigator.clipboard.writeText(term.getSelection());
        return false;
      }
      return true;
    });

    new ResizeObserver(fitNow).observe(el);
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      if (term) term.options.theme = theme();
    });

    fitNow();
    void start();
  }

  async function start() {
    if (!term) return;
    exited = false;
    term.reset();
    const ch = new Channel<TerminalEvent>();
    ch.onmessage = (ev) => {
      if (ch !== channel) return; // output from a session that was replaced
      if (ev.kind === "output") {
        term?.write(ev.data);
      } else {
        exited = true;
        if (Date.now() - startedAt > QUICK_EXIT_MS) onClose();
      }
    };
    channel = ch;
    startedAt = Date.now();
    try {
      await api.terminalStart(term.cols, term.rows, ch);
    } catch (e) {
      exited = true;
      toastError(e);
    }
  }

  // Created on first open; refit and focus on every open.
  $effect(() => {
    if (!open || !host) return;
    if (!term) init(host);
    // Untracked: only opening restarts, not the exit itself (a failed start
    // would otherwise retry in a loop).
    else if (untrack(() => exited)) void start();
    requestAnimationFrame(() => {
      fitNow();
      term?.focus();
    });
  });

  $effect(() => () => void api.terminalStop().catch(() => {}));

  function resetWidth() {
    width = clampWidth(DEFAULT_W);
    try {
      localStorage.setItem(WIDTH_KEY, String(width));
    } catch {
      // width just won't be remembered
    }
  }

  function onHandleDown(e: PointerEvent) {
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onHandleMove(e: PointerEvent) {
    if (dragging) width = clampWidth(window.innerWidth - e.clientX);
  }

  function onHandleUp() {
    if (!dragging) return;
    dragging = false;
    try {
      localStorage.setItem(WIDTH_KEY, String(width));
    } catch {
      // width just won't be remembered
    }
  }
</script>

<aside class="terminal-drawer" class:open class:fill class:dragging style="--w:{width}px" aria-hidden={!open}>
  {#if !fill}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="handle"
      onpointerdown={onHandleDown}
      onpointermove={onHandleMove}
      onpointerup={onHandleUp}
      onpointercancel={onHandleUp}
      ondblclick={resetWidth}
      title="Drag to resize · double-click to reset"
    ></div>
  {/if}
  <div class="inner">
    <header data-tauri-drag-region>
      <Icon name="terminal" size={13} />
      <span class="title" data-tauri-drag-region>Claude</span>
      <span class="spacer" data-tauri-drag-region></span>
      <button class="icon-btn" title="Restart Claude" onclick={() => void start()}>
        <Icon name="rotateCcw" size={13} />
      </button>
      <button class="icon-btn" title="Hide (⌃`) — Claude keeps running" onclick={onClose}>
        <Icon name="x" size={14} />
      </button>
    </header>
    <div class="term" bind:this={host}></div>
    {#if exited}
      <div class="exited">
        <span>Claude exited.</span>
        <button class="btn primary" onclick={() => void start()}>Restart</button>
      </div>
    {/if}
  </div>
</aside>

<style>
  .terminal-drawer {
    position: relative;
    flex-shrink: 0;
    width: 0;
    overflow: hidden;
    border-left: 1px solid transparent;
    background: var(--panel);
    transition: width 180ms ease;
  }
  .terminal-drawer.open {
    width: var(--w);
    border-left-color: var(--border);
  }
  .terminal-drawer.open.fill {
    flex: 1;
    width: auto;
    min-width: 0;
    transition: none;
  }
  .fill .inner {
    width: 100%;
  }
  .terminal-drawer.dragging {
    transition: none;
    user-select: none;
  }
  /* Fixed width so sliding doesn't reflow the terminal on every frame. */
  .inner {
    width: var(--w);
    height: 100%;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .handle {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 2;
  }
  .handle:hover,
  .dragging .handle {
    background: var(--accent-soft);
  }
  header {
    height: var(--titlebar-h);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 8px 0 14px;
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-size: 12px;
    font-weight: 600;
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .icon-btn {
    display: inline-flex;
    padding: 5px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
  }
  .icon-btn:hover {
    background: var(--panel-2);
    color: var(--text);
  }
  .term {
    flex: 1;
    min-height: 0;
    padding: 6px 4px 6px 12px;
  }
  .exited {
    position: absolute;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 14px;
    background: var(--panel-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    font-size: 12px;
    color: var(--text-dim);
  }
</style>
