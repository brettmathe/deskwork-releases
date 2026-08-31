<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type RepoCandidate, type SetupResult, type Tooling } from "./api";
  import Icon from "./Icon.svelte";

  let { onReady }: { onReady: (root: string) => void } = $props();

  /// Deliberately empty: release builds are published to a public repo, and a
  /// hardcoded default would ship the private workspace repo's path inside the
  /// bundle for anyone to read.
  const DEFAULT_URL = "";

  type Step = "choose" | "clone" | "create" | "done";

  let step = $state<Step>("choose");
  let scanning = $state(true);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let candidates = $state<RepoCandidate[]>([]);
  let tooling = $state<Tooling | null>(null);
  let result = $state<SetupResult | null>(null);

  let cloneUrl = $state(DEFAULT_URL);
  let cloneParent = $state("");
  let cloneFolder = $state("mywork");
  let cloneFolderTouched = $state(false);

  let createParent = $state("");
  let createFolder = $state("mywork");
  let initGit = $state(true);

  // Keep the clone folder in step with the URL until the user overrides it.
  $effect(() => {
    if (cloneFolderTouched) return;
    const tail = cloneUrl.trim().replace(/\/+$/, "").split(/[/:]/).pop() ?? "";
    cloneFolder = tail.replace(/\.git$/, "") || "mywork";
  });

  async function boot() {
    scanning = true;
    try {
      const [found, tools] = await Promise.all([api.detectRepos(), api.checkTooling()]);
      candidates = found;
      tooling = tools;
      cloneParent = tools.defaultParent;
      createParent = tools.defaultParent;
    } catch (e) {
      error = String(e);
    } finally {
      scanning = false;
    }
  }
  void boot();

  function goto(next: Step) {
    error = null;
    step = next;
  }

  /// Run a setup action, funnelling failures into the inline error slot rather
  /// than a toast — the wizard is the only thing on screen.
  async function attempt(label: string, fn: () => Promise<SetupResult | string>) {
    busy = label;
    error = null;
    try {
      const outcome = await fn();
      if (typeof outcome === "string") {
        onReady(outcome);
        return;
      }
      result = outcome;
      if (outcome.warning) {
        step = "done";
      } else {
        onReady(outcome.path);
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function browse(title: string): Promise<string | null> {
    const dir = await open({ directory: true, title });
    return typeof dir === "string" ? dir : null;
  }

  async function useCandidate(path: string) {
    await attempt("adopt", () => api.setRepoRoot(path));
  }

  async function pickExisting() {
    const dir = await browse("Choose your workspace folder");
    if (dir) await attempt("adopt", () => api.setRepoRoot(dir));
  }

  async function doClone() {
    await attempt("clone", () => api.cloneRepo(cloneParent, cloneUrl, cloneFolder));
  }

  async function doCreate() {
    await attempt("create", () => api.createRepo(createParent, createFolder, initGit));
  }

  const home = $derived(tooling?.defaultParent.replace(/\/[^/]*$/, "") ?? "");
  const short = (p: string) => (home && p.startsWith(home + "/") ? "~" + p.slice(home.length) : p);
</script>

<div class="wizard">
  <header>
    {#if step !== "choose" && step !== "done"}
      <button class="back" onclick={() => goto("choose")} disabled={!!busy}>
        <Icon name="chevronLeft" size={14} /> Back
      </button>
    {/if}
    <h1>Deskwork</h1>
    <p class="lede">
      {#if step === "choose"}
        Deskwork stores everything as markdown in a workspace folder — one with
        <span class="mono">inbox/</span> and <span class="mono">projects/</span> inside.
      {:else if step === "clone"}
        Clone an existing workspace from a git remote.
      {:else if step === "create"}
        Create a new workspace from scratch.
      {:else}
        Workspace ready.
      {/if}
    </p>
  </header>

  {#if error}
    <div class="error" role="alert">{error}</div>
  {/if}

  {#if step === "choose"}
    {#if scanning}
      <p class="hint">Looking for a workspace…</p>
    {:else if candidates.length > 0}
      <h2>Found on this Mac</h2>
      <ul class="candidates">
        {#each candidates as c (c.path)}
          <li>
            <button class="candidate" onclick={() => useCandidate(c.path)} disabled={!!busy}>
              <Icon name="folder" size={15} />
              <span class="col">
                <span class="path">{short(c.path)}</span>
                {#if c.remote}<span class="sub mono">{c.remote}</span>
                {:else if !c.hasGit}<span class="sub">not a git repository</span>{/if}
              </span>
              <span class="use">Use</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <h2>{candidates.length > 0 ? "Or" : "Get started"}</h2>
    <div class="options">
      <button class="option" onclick={pickExisting} disabled={!!busy}>
        <Icon name="search" size={16} />
        <span class="col">
          <b>Choose a folder…</b>
          <span class="sub">Point at a workspace you already have</span>
        </span>
      </button>
      <button class="option" onclick={() => goto("clone")} disabled={!!busy}>
        <Icon name="download" size={16} />
        <span class="col">
          <b>Clone from a remote</b>
          <span class="sub">Download an existing workspace from git</span>
        </span>
      </button>
      <button class="option" onclick={() => goto("create")} disabled={!!busy}>
        <Icon name="folderPlus" size={16} />
        <span class="col">
          <b>Create a new workspace</b>
          <span class="sub">Scaffold the folders and start a git repository</span>
        </span>
      </button>
    </div>

    {#if tooling && !tooling.git}
      <p class="warn">
        <b>git isn't installed.</b> Cloning, creating, and syncing all need it — install the Xcode
        Command Line Tools with <span class="mono">xcode-select --install</span>.
      </p>
    {/if}
  {/if}

  {#if step === "clone"}
    <label class="field">
      <span>Repository URL</span>
      <input
        type="text"
        bind:value={cloneUrl}
        spellcheck="false"
        disabled={!!busy}
        placeholder="https://github.com/owner/repo.git"
      />
    </label>
    <label class="field">
      <span>Clone into</span>
      <div class="row">
        <span class="mono grow">{short(cloneParent) || "choose a folder"}</span>
        <button class="btn" onclick={async () => { const d = await browse("Choose where to clone"); if (d) cloneParent = d; }} disabled={!!busy}>
          Browse…
        </button>
      </div>
    </label>
    <label class="field">
      <span>Folder name</span>
      <input
        type="text"
        bind:value={cloneFolder}
        spellcheck="false"
        disabled={!!busy}
        oninput={() => (cloneFolderTouched = true)}
      />
    </label>
    <p class="hint">
      {#if cloneUrl.trim()}Creates <span class="mono">{short(cloneParent)}/{cloneFolder}</span>.{/if}
      {#if tooling?.ghAuthed}
        Private repos will use your authenticated <span class="mono">gh</span> CLI.
      {:else if tooling?.gh}
        The <span class="mono">gh</span> CLI is installed but not signed in — run
        <span class="mono">gh auth login</span> first if this repository is private.
      {:else}
        For a private repository, install and sign in to the GitHub CLI
        (<span class="mono">gh auth login</span>) or make sure your SSH key is loaded.
      {/if}
    </p>
    <div class="actions">
      <button class="btn primary" onclick={doClone} disabled={!!busy || !cloneUrl.trim() || !cloneParent}>
        {busy === "clone" ? "Cloning…" : "Clone"}
      </button>
    </div>
  {/if}

  {#if step === "create"}
    <label class="field">
      <span>Create in</span>
      <div class="row">
        <span class="mono grow">{short(createParent) || "choose a folder"}</span>
        <button class="btn" onclick={async () => { const d = await browse("Choose where to create the workspace"); if (d) createParent = d; }} disabled={!!busy}>
          Browse…
        </button>
      </div>
    </label>
    <label class="field">
      <span>Folder name</span>
      <input type="text" bind:value={createFolder} spellcheck="false" disabled={!!busy} />
    </label>

    <div class="preview">
      <span class="mono root">{short(createParent)}/{createFolder || "…"}</span>
      <ul class="mono">
        <li>inbox/<span class="note">open tasks</span></li>
        <li>completed/<span class="note">finished tasks</span></li>
        <li>projects/<span class="note">registry table + one folder per project</span></li>
        <li>projects/_template.md<span class="note">starting point for a new project</span></li>
        <li>README.md<span class="note">how the layout works</span></li>
      </ul>
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={initGit} disabled={!!busy} />
      <span>
        <b>Initialize a git repository</b>
        <span class="sub">
          Runs <span class="mono">git init</span> on <span class="mono">main</span> and makes the
          first commit, so the Repository panel can track and sync your changes. Add a remote later
          with <span class="mono">git remote add origin …</span>.
        </span>
      </span>
    </label>

    {#if initGit && tooling && !tooling.gitIdentity}
      <p class="warn">
        Git has no <span class="mono">user.name</span> / <span class="mono">user.email</span> set,
        so the folder will be created and initialized but left uncommitted.
      </p>
    {/if}

    <div class="actions">
      <button class="btn primary" onclick={doCreate} disabled={!!busy || !createFolder.trim() || !createParent}>
        {busy === "create" ? "Creating…" : "Create workspace"}
      </button>
    </div>
  {/if}

  {#if step === "done" && result}
    <p class="hint">
      Workspace at <span class="mono">{short(result.path)}</span>.
    </p>
    {#if result.warning}
      <p class="warn">{result.warning}</p>
    {/if}
    <div class="actions">
      <button class="btn primary" onclick={() => onReady(result!.path)}>Continue</button>
    </div>
  {/if}
</div>

<style>
  .wizard {
    width: 560px;
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 64px);
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: var(--shadow);
    padding: 24px 26px 22px;
  }
  header {
    position: relative;
  }
  h1 {
    font-size: 20px;
    margin: 0 0 4px;
  }
  h2 {
    font-size: 11px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    margin: 20px 0 8px;
  }
  .lede,
  .hint {
    color: var(--text-dim);
    font-size: 12.5px;
    margin: 0 0 4px;
    line-height: 1.5;
  }
  .back {
    float: right;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    color: var(--text-dim);
    padding: 2px 4px;
  }
  .back:hover {
    color: var(--text);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.92em;
  }
  .sub {
    display: block;
    color: var(--text-dim);
    font-size: 11.5px;
    font-weight: 400;
    line-height: 1.45;
  }

  .candidates {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .candidate,
  .option {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    text-align: left;
    padding: 10px 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    transition: border-color 0.12s, background 0.12s;
  }
  .candidate:hover:not(:disabled),
  .option:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .candidate:disabled,
  .option:disabled {
    opacity: 0.55;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .col {
    flex: 1;
    min-width: 0;
  }
  .path {
    display: block;
    font-size: 13px;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .candidate .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .use {
    font-size: 11.5px;
    color: var(--accent);
    font-weight: 600;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .option b {
    font-size: 13px;
    font-weight: 550;
  }

  .field {
    display: block;
    margin: 14px 0;
  }
  .field > span {
    display: block;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-dim);
    margin-bottom: 5px;
  }
  .field input[type="text"] {
    width: 100%;
    padding: 7px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    font-size: 12.5px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .field input[type="text"]::placeholder {
    color: var(--text-dim);
    opacity: 0.6;
  }
  .field input[type="text"]:focus {
    outline: none;
    border-color: var(--accent);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--panel-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: 5px 6px 5px 10px;
  }
  .grow {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview {
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 11px 14px;
    margin: 14px 0;
  }
  .preview .root {
    font-size: 12.5px;
    font-weight: 600;
  }
  .preview ul {
    list-style: none;
    margin: 7px 0 0;
    padding: 0 0 0 14px;
    border-left: 1px solid var(--border-strong);
  }
  .preview li {
    font-size: 12px;
    color: var(--text-dim);
    padding: 1.5px 0;
  }
  .preview .note {
    font-family: var(--font, inherit);
    font-size: 11px;
    opacity: 0.72;
    margin-left: 8px;
  }

  .check {
    display: flex;
    gap: 9px;
    align-items: flex-start;
    margin: 16px 0 4px;
    font-size: 12.5px;
  }
  .check input {
    margin-top: 2px;
  }
  .check b {
    font-weight: 550;
  }

  .error,
  .warn {
    border-radius: var(--radius-sm);
    padding: 9px 12px;
    font-size: 12px;
    line-height: 1.5;
    margin: 12px 0 0;
    white-space: pre-wrap;
  }
  .error {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .warn {
    background: var(--accent-soft);
    color: var(--text-dim);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 18px;
  }
</style>
