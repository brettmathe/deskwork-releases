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
  import UpdateBanner from "$lib/UpdateBanner.svelte";
  import { checkForUpdate, loadCurrentVersion, updater } from "$lib/updater.svelte";
  import Toasts from "$lib/Toasts.svelte";

  type Nav = "inbox" | "projects" | "settings";

  let repoRoot: string | null = $state(null);
  let booted = $state(false);
  let nav: Nav = $state("inbox");

  let inboxItems: InboxItem[] = $state([]);
  let inboxExtras: ExtraEntry[] = $state([]);
  let selectedItem: InboxItemFull | null = $state(null);
  let editorDirty = $state(false);
  let showNewModal = $state(false);

  let projects: ProjectSummary[] = $state([]);
  let selectedProject: ProjectDetailT | null = $state(null);
  let gitPanel: GitPanel | undefined = $state();
  let terminalOpen = $state(false);

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
    if (e.ctrlKey && e.key === "`" && repoRoot) {
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
  <div class="shell">
    <nav class="sidebar">
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
      <div class="list-pane">
        <InboxList
          items={inboxItems}
          extras={inboxExtras}
          selected={selectedItem?.filename ?? null}
          onSelect={selectItem}
          onNew={() => (showNewModal = true)}
        />
      </div>
      <div class="detail-pane">
        <InboxDetail
          item={selectedItem}
          onChanged={onItemChanged}
          onGone={onItemGone}
          onDirty={(d) => (editorDirty = d)}
        />
      </div>
    {:else if nav === "projects"}
      <div class="list-pane">
        <ProjectsList {projects} selected={selectedProject?.dir ?? null} onSelect={selectProject} />
      </div>
      <div class="detail-pane">
        <ProjectDetail project={selectedProject} />
      </div>
    {:else}
      <div class="detail-pane single">
        <Settings {repoRoot} {onRootChanged} />
      </div>
    {/if}
    <TerminalDrawer open={terminalOpen} onClose={() => (terminalOpen = false)} />
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
  }
  .detail-pane {
    flex: 1;
    min-width: 0;
  }
  .setup {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
