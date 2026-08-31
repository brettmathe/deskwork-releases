<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import { toast, toastError } from "./toast.svelte";
  import {
    checkForUpdate,
    installUpdate,
    loadCurrentVersion,
    restartApp,
    updater,
  } from "./updater.svelte";

  void loadCurrentVersion();

  async function manualCheck() {
    const found = await checkForUpdate();
    if (!found && updater.status === "uptodate") toast("Deskwork is up to date.");
  }

  let {
    repoRoot,
    onRootChanged,
  }: {
    repoRoot: string | null;
    onRootChanged: (root: string) => void;
  } = $props();

  async function pickRoot() {
    try {
      const dir = await open({ directory: true, title: "Choose your workspace folder" });
      if (typeof dir !== "string") return;
      const confirmed = await api.setRepoRoot(dir);
      toast(`Repository set to ${confirmed}`);
      onRootChanged(confirmed);
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="pane">
  <header data-tauri-drag-region>
    <h2 data-tauri-drag-region>Settings</h2>
  </header>
  <div class="body">
    <section>
      <h3>Repository</h3>
      <p class="hint">
        Deskwork reads and writes markdown files in this repo. Use the Repository panel in the
        sidebar to pull from origin and to commit &amp; push task changes; it only ever commits
        files under inbox/ and completed/.
      </p>
      <div class="root-row">
        <span class="mono">{repoRoot ?? "not set"}</span>
        <button class="btn" onclick={pickRoot}>Change…</button>
      </div>
    </section>
    <section>
      <h3>Updates</h3>
      <p class="hint">
        Deskwork checks for a new version on launch and installs it on your say-so. Updates are
        signed; a build that fails signature verification is refused.
      </p>
      <div class="root-row">
        <span class="mono">Version {updater.currentVersion ?? "…"}</span>
        {#if updater.status === "available"}
          <button class="btn primary" onclick={() => void installUpdate()}>
            Install {updater.version}
          </button>
        {:else if updater.status === "downloading"}
          <button class="btn" disabled>
            {updater.progress === null
              ? "Downloading…"
              : `Downloading ${Math.round(updater.progress * 100)}%`}
          </button>
        {:else if updater.status === "ready"}
          <button class="btn primary" onclick={() => void restartApp()}>Restart to finish</button>
        {:else}
          <button class="btn" onclick={manualCheck} disabled={updater.status === "checking"}>
            {updater.status === "checking" ? "Checking…" : "Check for updates"}
          </button>
        {/if}
      </div>
      {#if updater.status === "error" && updater.error}
        <p class="err">{updater.error}</p>
      {/if}
      {#if updater.status === "available" && updater.notes}
        <p class="hint notes">{updater.notes}</p>
      {/if}
    </section>
    <section>
      <h3>How actions map to files</h3>
      <ul class="hint">
        <li><b>Complete</b> moves the file from <span class="mono">inbox/</span> to <span class="mono">completed/</span> (git sees a rename).</li>
        <li><b>Create / Edit</b> writes atomically inside <span class="mono">inbox/</span>.</li>
        <li><b>Delete</b> removes the file — recoverable via git until committed.</li>
        <li>Projects are read-only.</li>
      </ul>
    </section>
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    padding: calc(var(--titlebar-h) - 26px) 20px 10px;
    min-height: var(--titlebar-h);
    border-bottom: 1px solid var(--border);
  }
  h2 {
    font-size: 15px;
    font-weight: 650;
    margin: 0;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 22px 26px;
    max-width: 640px;
  }
  section {
    margin-bottom: 28px;
  }
  h3 {
    font-size: 13.5px;
    font-weight: 600;
    margin: 0 0 6px;
  }
  .hint {
    color: var(--text-dim);
    font-size: 12.5px;
    margin: 0 0 10px;
  }
  ul.hint {
    padding-left: 18px;
  }
  ul.hint li {
    margin: 4px 0;
  }
  .root-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 14px;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    margin: 8px 0 0;
    white-space: pre-wrap;
  }
  .notes {
    margin-top: 8px;
    white-space: pre-wrap;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .root-row .mono {
    font-size: 12px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
</style>
