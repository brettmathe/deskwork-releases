<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import type {
    ExtraEntry,
    InboxItem,
    InboxItemFull,
    ProjectDetail as ProjectDetailT,
    ProjectSummary,
  } from "$lib/api";
  import { api } from "$lib/api";
  import { toastError } from "$lib/toast.svelte";
  import Icon from "$lib/Icon.svelte";
  import InboxList from "$lib/InboxList.svelte";
  import InboxDetail from "$lib/InboxDetail.svelte";
  import NewItemModal from "$lib/NewItemModal.svelte";
  import ProjectsList from "$lib/ProjectsList.svelte";
  import ProjectDetail from "$lib/ProjectDetail.svelte";
  import GitPanel from "$lib/GitPanel.svelte";
  import Settings from "$lib/Settings.svelte";
  import SetupWizard from "$lib/SetupWizard.svelte";
  import TerminalDrawer from "$lib/TerminalDrawer.svelte";
  import ResizeHandle from "$lib/ResizeHandle.svelte";
  import UpdateBanner from "$lib/UpdateBanner.svelte";
  import { checkForUpdate, loadCurrentVersion, updater } from "$lib/updater.svelte";
  import Toasts from "$lib/Toasts.svelte";

  type Nav = "inbox" | "projects" | "settings";

  let repoRoot: string | null = $state(null);
  let booted = $state(false);
  let nav = $state<Nav>("inbox");

  let inboxItems: InboxItem[] = $state([]);
  let inboxExtras: ExtraEntry[] = $state([]);
  let selectedItem: InboxItemFull | null = $state(null);
  let editorDirty = $state(false);
  let showNewModal = $state(false);

  let projects: ProjectSummary[] = $state([]);
  let selectedProject: ProjectDetailT | null = $state(null);
  let gitPanel: GitPanel | undefined = $state();
  let terminalOpen = $state(false);

  // Which panels are expanded; each collapses to zero width. Remembered per machine.
  type Panel = "sidebar" | "list" | "detail";
  const PANELS_KEY = "deskwork.panels";
  let shown: Record<Panel, boolean> = $state(loadPanels());

  function loadPanels(): Record<Panel, boolean> {
    const all = { sidebar: true, list: true, detail: true };
    try {
      return { ...all, ...JSON.parse(localStorage.getItem(PANELS_KEY) ?? "{}") };
    } catch {
      return all;
    }
  }

  function setPanel(p: Panel, value: boolean) {
    shown[p] = value;
    try {
      localStorage.setItem(PANELS_KEY, JSON.stringify(shown));
    } catch {
      // layout just won't be remembered
    }
  }

  // Sidebar and list widths are user-set; the viewer takes what's left.
  // Sidebar minimum keeps the panel toggles inside it.
  type Sized = "sidebar" | "list";
  const WIDTHS_KEY = "deskwork.widths";
  const WIDTH_LIMITS: Record<Sized, { min: number; max: number; initial: number }> = {
    sidebar: { min: 190, max: 360, initial: 200 },
    list: { min: 240, max: 640, initial: 330 },
  };
  let widths: Record<Sized, number> = $state(loadWidths());
  let resizing = $state(false);
  let dragFrom = 0;

  function loadWidths(): Record<Sized, number> {
    const w = { sidebar: WIDTH_LIMITS.sidebar.initial, list: WIDTH_LIMITS.list.initial };
    try {
      const saved = JSON.parse(localStorage.getItem(WIDTHS_KEY) ?? "{}");
      for (const k of ["sidebar", "list"] as const) {
        if (typeof saved[k] === "number") w[k] = clampWidth(k, saved[k]);
      }
    } catch {
      // defaults
    }
    return w;
  }

  function clampWidth(k: Sized, w: number): number {
    return Math.round(Math.max(WIDTH_LIMITS[k].min, Math.min(WIDTH_LIMITS[k].max, w)));
  }

  function saveWidths() {
    try {
      localStorage.setItem(WIDTHS_KEY, JSON.stringify(widths));
    } catch {
      // widths just won't be remembered
    }
  }

  function resizeHandlers(k: Sized) {
    return {
      onStart: () => {
        resizing = true;
        dragFrom = widths[k];
      },
      onDrag: (dx: number) => (widths[k] = clampWidth(k, dragFrom + dx)),
      onEnd: () => {
        resizing = false;
        saveWidths();
      },
      onReset: () => {
        widths[k] = WIDTH_LIMITS[k].initial;
        saveWidths();
      },
    };
  }

  // Settings has no list, and always shows its page.
  const hasList = $derived(nav !== "settings");
  const listVisible = $derived(hasList && shown.list);
  const detailVisible = $derived(nav === "settings" || shown.detail);
  // With the viewer collapsed, whatever is to its left or right takes the room.
  const terminalFill = $derived(terminalOpen && !detailVisible);
  const listFill = $derived(listVisible && !detailVisible && !terminalOpen);
  // With the sidebar collapsed, the leftmost pane's header makes room for the
  // traffic lights and the panel toggles.
  const lead = $derived(
    shown.sidebar ? null : listVisible ? "list" : detailVisible ? "detail" : terminalOpen ? "terminal" : null,
  );

  async function boot() {
    try {
      repoRoot = await api.getRepoRoot();
      if (repoRoot) await refreshAll();
    } catch (e) {
      toastError(e);
    } finally {
      booted = true;
    }
    // Silent so a dev build, an offline machine, or an unreachable endpoint
    // never greets someone with an error they didn't ask for.
    void loadCurrentVersion();
    void checkForUpdate(true);
  }

  async function refreshAll() {
    await Promise.all([refreshInbox(), refreshProjects()]);
  }

  async function refreshInbox() {
    try {
      [inboxItems, inboxExtras] = await Promise.all([api.listInbox(), api.listInboxExtras()]);
    } catch (e) {
      toastError(e);
    }
  }

  async function refreshProjects() {
    try {
      projects = await api.listProjects();
    } catch (e) {
      toastError(e);
    }
  }

  async function guardDirty(): Promise<boolean> {
    if (!editorDirty) return true;
    return confirm("Discard unsaved changes?", { title: "Unsaved changes", kind: "warning" });
  }

  async function selectItem(filename: string) {
    if (!(await guardDirty())) return;
    try {
      selectedItem = await api.getInboxItem(filename);
      if (!shown.detail) setPanel("detail", true);
    } catch (e) {
      toastError(e);
    }
  }

  async function onItemChanged() {
    if (selectedItem) {
      try {
        selectedItem = await api.getInboxItem(selectedItem.filename);
      } catch {
        selectedItem = null;
      }
    }
    await refreshInbox();
    void gitPanel?.refresh(false);
  }

  async function onItemGone(filename: string) {
    if (selectedItem?.filename === filename) selectedItem = null;
    await refreshInbox();
    void gitPanel?.refresh(false);
  }

  async function onCreated(item: InboxItem) {
    showNewModal = false;
    await refreshInbox();
    await selectItem(item.filename);
    void gitPanel?.refresh(false);
  }

  async function selectProject(dir: string) {
    try {
      selectedProject = await api.getProject(dir);
      if (!shown.detail) setPanel("detail", true);
    } catch (e) {
      toastError(e);
    }
  }

  async function setNav(n: Nav) {
    if (n !== "inbox" && !(await guardDirty())) return;
    nav = n;
  }

  function onRootChanged(root: string) {
    repoRoot = root;
    void refreshAll();
  }

  // Capture phase so the toggle works while the terminal has focus.
  function onKeydownCapture(e: KeyboardEvent) {
    if (!repoRoot) return;
    const panel: Panel | null =
      e.metaKey && !e.shiftKey && !e.altKey && !e.ctrlKey
        ? ({ "1": "sidebar", "2": "list", "3": "detail" } as const)[e.key as "1" | "2" | "3"] ?? null
        : null;
    if (panel) {
      e.preventDefault();
      e.stopPropagation();
      setPanel(panel, !shown[panel]);
    } else if (e.ctrlKey && e.key === "`") {
      e.preventDefault();
      e.stopPropagation();
      terminalOpen = !terminalOpen;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Keys typed into the terminal belong to Claude, not to Deskwork.
    if ((e.target as Element | null)?.closest?.(".terminal-drawer")) return;
    if (e.metaKey && e.key === "n" && nav === "inbox" && repoRoot) {
      e.preventDefault();
      showNewModal = true;
    }
  }

  // Refresh from disk when the window regains focus (external edits, Slack scanner).
  function onFocus() {
    if (repoRoot && !editorDirty) {
      void refreshAll();
      void gitPanel?.refresh(true);
    }
  }

  // Files changed on disk while Deskwork stayed focused (e.g. Claude in the
  // terminal drawer). Same guard as the focus refresh; also reloads the open project.
  async function onWorkspaceChanged() {
    void gitPanel?.refresh(false);
    if (!repoRoot || editorDirty) return;
    await refreshAll();
    if (selectedProject) {
      try {
        selectedProject = await api.getProject(selectedProject.dir);
      } catch {
        selectedProject = null;
      }
    }
  }

  $effect(() => {
    const unlisten = listen("workspace-changed", () => void onWorkspaceChanged());
    return () => void unlisten.then((f) => f());
  });

  void boot();
</script>

<svelte:window onkeydown={onKeydown} onkeydowncapture={onKeydownCapture} onfocus={onFocus} />

{#if booted && !repoRoot}
  <div class="setup" data-tauri-drag-region>
    <SetupWizard onReady={onRootChanged} />
  </div>
{:else}
  <div class="layout-toggles">
    <button class:on={shown.sidebar} title="Sidebar (⌘1)" onclick={() => setPanel("sidebar", !shown.sidebar)}>
      <Icon name="panelLeft" size={14} />
    </button>
    <button
      class:on={listVisible}
      disabled={!hasList}
      title="List (⌘2)"
      onclick={() => setPanel("list", !shown.list)}
    >
      <Icon name="list" size={14} />
    </button>
    <button
      class:on={detailVisible}
      disabled={nav === "settings"}
      title="Viewer (⌘3)"
      onclick={() => setPanel("detail", !shown.detail)}
    >
      <Icon name="fileText" size={14} />
    </button>
    <button class:on={terminalOpen} title="Claude (⌃`)" onclick={() => (terminalOpen = !terminalOpen)}>
      <Icon name="terminal" size={14} />
    </button>
  </div>

  <div
    class="shell"
    class:lead-terminal={lead === "terminal"}
    class:resizing
    style="--sidebar-w:{widths.sidebar}px; --list-w:{widths.list}px"
  >
    <nav class="sidebar" class:collapsed={!shown.sidebar} aria-hidden={!shown.sidebar}>
      {#if shown.sidebar}<ResizeHandle {...resizeHandlers("sidebar")} />{/if}
      <div class="brand" data-tauri-drag-region>
        Deskwork{#if updater.currentVersion}<span class="version">{updater.currentVersion}</span>{/if}
      </div>
      <button class="nav-item" class:current={nav === "inbox"} onclick={() => setNav("inbox")}>
        <Icon name="inbox" size={15} />
        <span>Inbox</span>
        {#if inboxItems.length > 0}<span class="count">{inboxItems.length}</span>{/if}
      </button>
      <button class="nav-item" class:current={nav === "projects"} onclick={() => setNav("projects")}>
        <Icon name="folder" size={15} />
        <span>Projects</span>
      </button>
      <button
        class="nav-item"
        class:current={terminalOpen}
        title="Claude in this workspace (⌃`)"
        onclick={() => (terminalOpen = !terminalOpen)}
      >
        <Icon name="terminal" size={15} />
        <span>Claude</span>
      </button>
      <div class="spacer"></div>
      <UpdateBanner />
      <GitPanel bind:this={gitPanel} onPulled={() => void refreshAll()} />
      <button class="nav-item" class:current={nav === "settings"} onclick={() => setNav("settings")}>
        <Icon name="settings" size={15} />
        <span>Settings</span>
      </button>
    </nav>

    {#if nav === "inbox"}
      <div class="list-pane" class:collapsed={!listVisible} class:fill={listFill} class:lead={lead === "list"}>
        {#if listVisible && !listFill}<ResizeHandle {...resizeHandlers("list")} />{/if}
        <InboxList
          items={inboxItems}
          extras={inboxExtras}
          selected={selectedItem?.filename ?? null}
          onSelect={selectItem}
          onNew={() => (showNewModal = true)}
        />
      </div>
      <div class="detail-pane" class:collapsed={!detailVisible} class:lead={lead === "detail"}>
        <InboxDetail
          item={selectedItem}
          onChanged={onItemChanged}
          onGone={onItemGone}
          onDirty={(d) => (editorDirty = d)}
        />
      </div>
    {:else if nav === "projects"}
      <div class="list-pane" class:collapsed={!listVisible} class:fill={listFill} class:lead={lead === "list"}>
        {#if listVisible && !listFill}<ResizeHandle {...resizeHandlers("list")} />{/if}
        <ProjectsList {projects} selected={selectedProject?.dir ?? null} onSelect={selectProject} />
      </div>
      <div class="detail-pane" class:collapsed={!detailVisible} class:lead={lead === "detail"}>
        <ProjectDetail project={selectedProject} />
      </div>
    {:else}
      <div class="detail-pane single" class:lead={lead === "detail"}>
        <Settings {repoRoot} {onRootChanged} />
      </div>
    {/if}
    {#if !listVisible && !detailVisible && !terminalOpen}
      <div class="all-collapsed" data-tauri-drag-region>
        All panels are collapsed. Use the toggles at the top left, or ⌘1 ⌘2 ⌘3 and ⌃`.
      </div>
    {/if}
    <TerminalDrawer open={terminalOpen} fill={terminalFill} onClose={() => (terminalOpen = false)} />
  </div>
{/if}

{#if showNewModal}
  <NewItemModal onClose={() => (showNewModal = false)} onCreated={onCreated} />
{/if}

<Toasts />

<style>
  .shell {
    display: flex;
    height: 100vh;
  }
  .sidebar {
    width: var(--sidebar-w);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: 0 8px 10px;
    border-right: 1px solid var(--border);
    overflow: hidden;
    position: relative;
    transition:
      width 160ms ease,
      padding 160ms ease;
  }
  /* No easing while dragging a handle, or the pane lags the pointer. */
  .shell.resizing .sidebar,
  .shell.resizing .list-pane {
    transition: none;
  }
  .shell.resizing {
    user-select: none;
    cursor: col-resize;
  }
  .sidebar.collapsed {
    width: 0;
    padding-left: 0;
    padding-right: 0;
    border-right: none;
  }
  /* Next to the macOS traffic lights, in the titlebar band; always reachable,
     including with the sidebar collapsed. */
  .layout-toggles {
    position: fixed;
    top: calc((var(--titlebar-h) - 24px) / 2);
    left: 78px;
    z-index: 20;
    display: flex;
    gap: 2px;
  }
  .layout-toggles button {
    display: inline-flex;
    padding: 5px;
    border-radius: var(--radius-sm);
    color: var(--text-faint);
  }
  .layout-toggles button:hover:not(:disabled) {
    background: var(--panel-2);
    color: var(--text);
  }
  .layout-toggles button.on {
    color: var(--text-dim);
  }
  .layout-toggles button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  /* Traffic lights + toggles span ~190px; the leftmost pane's header starts after them. */
  .lead :global(header),
  .lead-terminal :global(.terminal-drawer header) {
    padding-left: 190px;
  }
  .all-collapsed {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    color: var(--text-faint);
    font-size: 12px;
  }
  .brand {
    font-size: 13px;
    font-weight: 650;
    color: var(--text-dim);
    padding: 0 10px 12px;
    /* leave room for the macOS traffic lights (titlebar overlay) */
    padding-top: calc(var(--titlebar-h) + 8px);
  }
  /* Identity, not status -- kept quiet so it never competes with the nav or the
     update banner. Rendered only once the version resolves, so it cannot flash
     a placeholder on launch. */
  .brand .version {
    margin-left: 6px;
    font-size: 10px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    opacity: 0.65;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
    font-weight: 500;
    margin-bottom: 1px;
  }
  .nav-item:hover {
    background: var(--panel-2);
    color: var(--text);
  }
  .nav-item.current {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .nav-item .count {
    margin-left: auto;
    font-size: 11px;
    font-weight: 600;
    background: var(--panel-2);
    border-radius: 99px;
    padding: 0 7px;
    color: var(--text-dim);
  }
  .nav-item.current .count {
    background: transparent;
  }
  .spacer {
    flex: 1;
  }
  .list-pane {
    width: var(--list-w);
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    min-width: 0;
    overflow: hidden;
    position: relative;
    transition: width 160ms ease;
  }
  .list-pane.fill {
    flex: 1;
    width: auto;
    border-right: none;
  }
  .list-pane.collapsed {
    width: 0;
    border-right: none;
  }
  .detail-pane {
    flex: 1;
    min-width: 0;
  }
  .detail-pane.collapsed {
    display: none;
  }
  .setup {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
