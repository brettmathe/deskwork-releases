<script lang="ts">
  import type { GitChange, GitStatus } from "./api";
  import { api } from "./api";
  import { toast, toastError } from "./toast.svelte";
  import Icon from "./Icon.svelte";

  let { onPulled }: { onPulled: () => void } = $props();

  let status: GitStatus | null = $state(null);
  let busy: "pull" | "push" | null = $state(null);
  let showCommitModal = $state(false);
  let message = $state("");

  export async function refresh(fetch: boolean) {
    try {
      status = await api.gitStatus(fetch);
      if (status.fetchError) {
        // Local status is still valid; surface fetch problems quietly.
        console.warn("git fetch failed:", status.fetchError);
      }
    } catch (e) {
      // Repo without git (or git missing) — hide the panel rather than nag.
      status = null;
      console.warn("git status unavailable:", e);
    }
  }

  // Periodic fetch so the behind-count tracks the CI bot commits.
  $effect(() => {
    void refresh(true);
    const t = setInterval(() => void refresh(true), 5 * 60 * 1000);
    return () => clearInterval(t);
  });

  function verbFor(c: GitChange): string {
    if (c.path.startsWith("completed/")) return "complete";
    switch (c.status) {
      case "untracked":
      case "added":
        return "add";
      case "deleted":
        return "delete";
      default:
        return "update";
    }
  }

  function stem(path: string): string {
    const name = path.split("/").pop() ?? path;
    return name.replace(/\.md$/, "").replace(/^\d{4}-\d{2}-\d{2}-/, "");
  }

  // projects/<slug>/... -> slug; projects/README.md -> "registry"; other top-level files by name.
  function projectKey(path: string): string {
    const parts = path.split("/");
    if (parts.length > 2) return parts[1];
    return parts[1] === "README.md" ? "registry" : stem(parts[1] ?? path);
  }

  function projectsMessage(changes: GitChange[]): string {
    const byKey = new Map<string, GitChange[]>();
    for (const c of changes) {
      const k = projectKey(c.path);
      byKey.set(k, [...(byKey.get(k) ?? []), c]);
    }
    const parts = [...byKey.entries()].map(([k, cs]) => {
      const verb = cs.every((c) => c.status === "untracked" || c.status === "added")
        ? "add"
        : cs.every((c) => c.status === "deleted")
          ? "remove"
          : "update";
      return `${verb} ${k}`;
    });
    return `Projects: ${parts.join("; ")}`;
  }

  function defaultMessage(tasks: GitChange[], projects: GitChange[]): string {
    const parts: string[] = [];
    if (tasks.length > 0) parts.push(tasksMessage(tasks));
    if (projects.length > 0) parts.push(projectsMessage(projects));
    let msg = parts.join(" · ");
    if (msg.length > 72) msg = `${msg.slice(0, 69)}…`;
    return msg;
  }

  function tasksMessage(changes: GitChange[]): string {
    // "complete" wins for a rename pair; dedupe by stem so inbox->completed
    // renames don't show twice.
    const seen = new Map<string, string>();
    for (const c of changes) {
      const s = stem(c.path);
      const v = verbFor(c);
      if (!seen.has(s) || v === "complete") seen.set(s, v);
    }
    let parts = [...seen.entries()].map(([s, v]) => `${v} ${s.replaceAll("-", " ")}`);
    return `Tasks: ${parts.join("; ")}`;
  }

  function openCommit() {
    if (!status) return;
    message =
      pending.length > 0
        ? defaultMessage(status.taskChanges, status.projectChanges)
        : "Push pending commits";
    showCommitModal = true;
  }

  async function commitPush() {
    if (busy) return;
    busy = "push";
    try {
      const result = await api.gitCommitPush(message);
      toast(`Sync: ${result}`);
      showCommitModal = false;
      await refresh(true);
      if (result.includes("pulled")) onPulled();
    } catch (e) {
      toastError(e);
    } finally {
      busy = null;
    }
  }

  async function pull() {
    if (busy) return;
    busy = "pull";
    try {
      const result = await api.gitPull();
      toast(result.split("\n")[0]);
      await refresh(false);
      onPulled();
    } catch (e) {
      toastError(e);
    } finally {
      busy = null;
    }
  }

  // Everything Commit & Push will stage: tasks first, then projects.
  const pending = $derived.by(() => {
    const s = status;
    return s ? [...s.taskChanges, ...s.projectChanges] : [];
  });

  const canPush = $derived.by(() => {
    const s = status;
    return s !== null && s.hasUpstream && (pending.length > 0 || s.ahead > 0);
  });

  // In sync only when nothing is waiting anywhere, including changes Deskwork won't commit.
  const inSync = $derived.by(() => {
    const s = status;
    return s !== null && s.ahead === 0 && s.behind === 0 && pending.length === 0 && s.otherChanges === 0;
  });
</script>

{#if status}
  <div class="git">
    <div class="git-title">Repository</div>
    <div class="git-line">
      <span class="branch mono">{status.branch}</span>
      {#if status.ahead > 0}<span class="counter ahead" title="{status.ahead} commit(s) to push">↑{status.ahead}</span>{/if}
      {#if status.behind > 0}<span class="counter behind" title="{status.behind} commit(s) behind origin">↓{status.behind}</span>{/if}
      {#if inSync}
        <span class="insync" title="In sync with origin"><Icon name="check" size={11} /></span>
      {/if}
    </div>
    {#if status.taskChanges.length > 0}
      <div class="git-changes">
        {status.taskChanges.length} task change{status.taskChanges.length === 1 ? "" : "s"}
      </div>
    {/if}
    {#if status.projectChanges.length > 0}
      <div class="git-changes" title={status.projectChanges.map((c) => c.path).join("\n")}>
        {status.projectChanges.length} project change{status.projectChanges.length === 1 ? "" : "s"}
      </div>
    {/if}
    {#if status.otherChanges > 0}
      <div class="git-other" title="Uncommitted changes outside inbox/, completed/ and projects/ — Deskwork won't commit these">
        +{status.otherChanges} elsewhere
      </div>
    {/if}

    {#if status.behind > 0}
      <button class="btn git-btn" onclick={pull} disabled={busy !== null}>
        {busy === "pull" ? "Pulling…" : `Pull ${status.behind} commit${status.behind === 1 ? "" : "s"}`}
      </button>
    {/if}
    {#if canPush}
      <button class="btn primary git-btn" onclick={openCommit} disabled={busy !== null}>
        {pending.length > 0 ? "Commit & Push" : `Push ${status.ahead}`}
      </button>
    {/if}
  </div>
{/if}

{#if showCommitModal && status}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="backdrop" onclick={(e) => e.target === e.currentTarget && (showCommitModal = false)}>
    <div class="modal">
      <h3>Commit &amp; push changes</h3>
      {#if pending.length > 0}
        <ul class="changes">
          {#each pending as c (c.path)}
            <li>
              <span class="pill {c.status === 'deleted' ? 'hold' : 'active'}">{verbFor(c)}</span>
              <span class="mono">{c.path}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="note">No file changes — pushing {status.ahead} pending commit{status.ahead === 1 ? "" : "s"}.</p>
      {/if}
      <input
        type="text"
        bind:value={message}
        placeholder="Commit message"
        onkeydown={(e) => e.key === "Enter" && commitPush()}
      />
      <p class="note">Pushes to origin/{status.branch}; pulls first if origin moved.</p>
      <div class="foot">
        <button class="btn" onclick={() => (showCommitModal = false)}>Cancel</button>
        <button
          class="btn primary"
          onclick={commitPush}
          disabled={busy !== null || (pending.length > 0 && !message.trim())}
        >
          {busy === "push" ? "Syncing…" : "Commit & Push"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .git {
    margin: 8px 2px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .git-title {
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }
  .git-line {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .branch {
    font-size: 11.5px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .counter {
    font-size: 11px;
    font-weight: 650;
    padding: 0 6px;
    border-radius: 99px;
  }
  .counter.behind {
    background: rgba(217, 119, 6, 0.14);
    color: #b45309;
  }
  @media (prefers-color-scheme: dark) {
    .counter.behind {
      color: #fbbf24;
    }
  }
  .counter.ahead {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .insync {
    color: var(--accent);
    display: inline-flex;
  }
  .git-changes {
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .git-other {
    font-size: 11px;
    color: var(--text-faint);
  }
  .git-btn {
    justify-content: center;
    font-size: 12px;
    padding: 4px 8px;
  }
  .git-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 15, 12, 0.35);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 14vh;
    z-index: 50;
  }
  .modal {
    width: 560px;
    max-width: calc(100vw - 48px);
    background: var(--panel);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: var(--shadow);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 650;
  }
  .changes {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 180px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .changes li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .changes .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  input {
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
    padding: 8px 10px;
    font-size: 13px;
    outline: none;
    user-select: text;
    cursor: text;
  }
  input:focus {
    border-color: var(--accent);
  }
  .note {
    margin: 0;
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
