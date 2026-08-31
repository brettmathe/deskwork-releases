<script lang="ts">
  import type { ProjectSummary } from "./api";
  import Icon from "./Icon.svelte";

  let {
    projects,
    selected,
    onSelect,
  }: {
    projects: ProjectSummary[];
    selected: string | null;
    onSelect: (dir: string) => void;
  } = $props();

  const active = $derived(projects.filter((p) => !p.onHold && !p.unregistered));
  const unregistered = $derived(projects.filter((p) => p.unregistered));
  const onHold = $derived(projects.filter((p) => p.onHold));

  function statusClass(status: string | null): string {
    const s = (status ?? "").toLowerCase();
    if (s.includes("hold") || s.includes("block") || s.includes("not started")) return "hold";
    if (s.includes("active")) return "active";
    return "";
  }
</script>

{#snippet row(p: ProjectSummary)}
  <button class="row" class:selected={p.dir === selected} onclick={() => onSelect(p.dir)}>
    <span class="chip" class:blank={p.priority === null}>{p.priority ?? "—"}</span>
    <span class="name">{p.name}</span>
    {#if p.registryStatus ?? p.fileStatus}
      <span class="pill {statusClass(p.registryStatus ?? p.fileStatus)}">
        {p.registryStatus ?? p.fileStatus}
      </span>
    {/if}
  </button>
  <div class="sub">
    {#if p.target}<span>{p.target}</span>{/if}
    {#if p.stakeholders}<span class="stake">{p.stakeholders}</span>{/if}
  </div>
{/snippet}

<div class="pane">
  <header data-tauri-drag-region>
    <h2 data-tauri-drag-region>Projects</h2>
  </header>

  <div class="rows">
    {#each active as p (p.dir)}
      {@render row(p)}
    {/each}

    {#if unregistered.length > 0}
      <div class="group">Not in registry</div>
      {#each unregistered as p (p.dir)}
        {@render row(p)}
      {/each}
    {/if}

    {#if onHold.length > 0}
      <div class="group">On hold</div>
      {#each onHold as p (p.dir)}
        {@render row(p)}
      {/each}
    {/if}

    {#if projects.length === 0}
      <div class="empty-state">
        <Icon name="folder" size={36} />
        <span>No projects found</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    padding: calc(var(--titlebar-h) - 26px) 14px 8px;
    min-height: var(--titlebar-h);
  }
  h2 {
    font-size: 15px;
    font-weight: 650;
    margin: 0;
    letter-spacing: -0.01em;
  }
  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 0 8px 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 8px 10px 2px;
    border-radius: var(--radius) var(--radius) 0 0;
  }
  .row:hover,
  .row:hover + .sub {
    background: var(--panel-2);
  }
  .row.selected,
  .row.selected + .sub {
    background: var(--accent-soft);
  }
  .name {
    flex: 1;
    font-weight: 550;
    font-size: 12.8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    display: flex;
    gap: 10px;
    padding: 0 10px 8px 40px;
    margin-bottom: 1px;
    border-radius: 0 0 var(--radius) var(--radius);
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .sub .stake {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .group {
    margin: 14px 10px 4px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }
</style>
