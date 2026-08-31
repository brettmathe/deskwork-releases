<script lang="ts">
  import type { ProjectDetail } from "./api";
  import { api } from "./api";
  import { renderMarkdown, handleMarkdownClick } from "./markdown";
  import { toastError } from "./toast.svelte";
  import Icon from "./Icon.svelte";

  let { project }: { project: ProjectDetail | null } = $props();

  // null = README; otherwise the relPath of the markdown artifact being viewed.
  let openArtifact: string | null = $state(null);
  let artifactContent = $state("");

  $effect(() => {
    project?.dir;
    openArtifact = null;
  });

  async function viewArtifact(relPath: string) {
    if (!project) return;
    try {
      artifactContent = await api.readArtifact(project.dir, relPath);
      openArtifact = relPath;
    } catch (e) {
      toastError(e);
    }
  }

  function openFile(relPath: string) {
    if (!project) return;
    api.openProjectFile(project.dir, relPath).catch(toastError);
  }

  function revealFile(relPath: string) {
    if (!project) return;
    api.revealProjectFile(project.dir, relPath).catch(toastError);
  }

  /** Resolve a relative markdown link against the current document's directory. */
  function resolveRel(href: string): string | null {
    const base = openArtifact ? openArtifact.split("/").slice(0, -1) : [];
    const parts = [...base];
    for (const seg of href.split("/")) {
      if (seg === "" || seg === ".") continue;
      if (seg === "..") {
        if (parts.length === 0) return null;
        parts.pop();
      } else {
        parts.push(seg);
      }
    }
    return parts.join("/");
  }

  function onRelativeLink(href: string) {
    const clean = href.split("#")[0];
    if (!clean) return;
    const resolved = resolveRel(clean);
    if (!resolved || !project) return;
    const target = resolved.replace(/\/$/, "");
    if (target.endsWith(".md")) {
      void viewArtifact(target);
    } else if (target.endsWith("README") === false && project.artifacts.some((a) => a.relPath.startsWith(target))) {
      openFile(target);
    }
  }

  const doc = $derived(
    project === null
      ? null
      : openArtifact === null
        ? project.readme
        : artifactContent,
  );
</script>

<div class="pane">
  {#if project && doc !== null}
    <header data-tauri-drag-region>
      <div class="crumbs" data-tauri-drag-region>
        {#if openArtifact}
          <button class="crumb-btn" onclick={() => (openArtifact = null)}>
            <Icon name="chevronLeft" size={13} />
            {project.name}
          </button>
          <span class="mono">{openArtifact}</span>
        {:else}
          <span class="crumb-current">{project.name}</span>
          <span class="mono">projects/{project.dir}/</span>
        {/if}
      </div>
      <button class="icon-btn" onclick={() => revealFile(openArtifact ?? "README.md")} title="Reveal in Finder">
        <Icon name="externalLink" size={13} />
      </button>
    </header>

    <div class="body">
      <div class="doc">
        {#if !openArtifact && project.metadata.length > 0}
          <div class="meta-card">
            {#each project.metadata as m (m.key)}
              <div class="meta-entry">
                <span class="meta-key">{m.key}</span>
                <span class="meta-val" class:pill={m.key.toLowerCase() === "status"} class:active={m.value.toLowerCase() === "active"}>{m.value}</span>
              </div>
            {/each}
          </div>
        {/if}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div onclick={(e) => handleMarkdownClick(e, onRelativeLink)}>
          <div class="md">{@html renderMarkdown(doc)}</div>
        </div>
      </div>

      <aside class="artifacts">
        <div class="aside-title">Documents</div>
        {#each project.artifacts as a (a.relPath)}
          {#if a.kind === "md"}
            <button
              class="artifact"
              class:selected={a.relPath === openArtifact}
              onclick={() => viewArtifact(a.relPath)}
            >
              <Icon name="fileText" size={12} />
              <span>{a.relPath}</span>
            </button>
          {:else}
            <button class="artifact ext" onclick={() => openFile(a.relPath)} title="Opens externally">
              <Icon name={a.kind === "html" ? "externalLink" : "file"} size={12} />
              <span>{a.relPath}</span>
            </button>
          {/if}
        {:else}
          <div class="no-artifacts">No documents besides the README.</div>
        {/each}
      </aside>
    </div>
  {:else}
    <div class="empty-state" data-tauri-drag-region>
      <Icon name="folder" size={36} />
      <span>Select a project</span>
    </div>
  {/if}
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: calc(var(--titlebar-h) - 26px) 20px 10px;
    min-height: var(--titlebar-h);
    border-bottom: 1px solid var(--border);
  }
  .crumbs {
    display: flex;
    align-items: baseline;
    gap: 10px;
    overflow: hidden;
    font-size: 12.5px;
  }
  .crumb-btn {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--accent);
    font-weight: 550;
    flex-shrink: 0;
  }
  .crumb-btn:hover {
    text-decoration: underline;
  }
  .crumb-current {
    font-weight: 600;
    flex-shrink: 0;
  }
  .crumbs .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .doc {
    flex: 1;
    overflow-y: auto;
    padding: 22px 26px 40px;
    min-width: 0;
  }
  .meta-card {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 26px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 14px;
    margin-bottom: 18px;
    max-width: 760px;
  }
  .meta-entry {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .meta-key {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
    font-weight: 600;
  }
  .meta-val {
    font-size: 12.5px;
  }
  .artifacts {
    width: 250px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    overflow-y: auto;
    padding: 14px 10px;
    background: var(--bg);
  }
  .aside-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
    padding: 0 8px 8px;
  }
  .artifact {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    text-align: left;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-dim);
  }
  .artifact span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artifact:hover {
    background: var(--panel-2);
    color: var(--text);
  }
  .artifact.selected {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .no-artifacts {
    padding: 8px;
    font-size: 12px;
    color: var(--text-faint);
  }
</style>
