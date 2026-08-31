<script lang="ts">
  import type { ExtraEntry, InboxItem } from "./api";
  import { api } from "./api";
  import { toastError } from "./toast.svelte";
  import Icon from "./Icon.svelte";

  let {
    items,
    extras,
    selected,
    onSelect,
    onNew,
  }: {
    items: InboxItem[];
    extras: ExtraEntry[];
    selected: string | null;
    onSelect: (filename: string) => void;
    onNew: () => void;
  } = $props();

  let filter = $state("");
  let showExtras = $state(false);

  const visible = $derived(
    filter.trim()
      ? items.filter((i) =>
          `${i.title}\n${i.preview}\n${i.filename}`.toLowerCase().includes(filter.trim().toLowerCase()),
        )
      : items,
  );

  function openExtra(name: string) {
    api.openInboxExtra(name).catch(toastError);
  }
</script>

<div class="pane">
  <header data-tauri-drag-region>
    <h2 data-tauri-drag-region>Inbox</h2>
    <button class="btn primary" onclick={onNew} title="New item (⌘N)">
      <Icon name="plus" size={13} />
      New
    </button>
  </header>

  <div class="search">
    <Icon name="search" size={13} />
    <input type="text" placeholder="Filter items…" bind:value={filter} />
  </div>

  <div class="rows">
    {#each visible as item (item.filename)}
      <button
        class="row"
        class:selected={item.filename === selected}
        onclick={() => onSelect(item.filename)}
      >
        <div class="row-top">
          <span class="title">{item.title}</span>
          {#if item.isSlack}
            <span class="slack" title="Created from Slack"><Icon name="slack" size={11} /></span>
          {/if}
        </div>
        <div class="row-mid">
          {#if item.preview}{item.preview}{:else}<i>No details</i>{/if}
        </div>
        <div class="row-bot">
          <span>{item.added ?? "no date"}</span>
          <span class="mono">{item.filename}</span>
        </div>
      </button>
    {:else}
      <div class="none">
        {items.length === 0 ? "Inbox zero — nothing here." : "No items match the filter."}
      </div>
    {/each}

    {#if extras.length > 0}
      <button class="extras-toggle" onclick={() => (showExtras = !showExtras)}>
        {showExtras ? "▾" : "▸"} Other files in inbox ({extras.length})
      </button>
      {#if showExtras}
        {#each extras as extra (extra.name)}
          <button class="extra" onclick={() => openExtra(extra.name)} title="Open externally">
            <Icon name={extra.isDir ? "folder" : "file"} size={12} />
            <span class="mono">{extra.name}{extra.isDir ? "/" : ""}</span>
            <Icon name="externalLink" size={11} />
          </button>
        {/each}
      {/if}
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
    justify-content: space-between;
    padding: calc(var(--titlebar-h) - 26px) 14px 8px;
    min-height: var(--titlebar-h);
  }
  h2 {
    font-size: 15px;
    font-weight: 650;
    margin: 0;
    letter-spacing: -0.01em;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 0 14px 8px;
    padding: 5px 10px;
    background: var(--panel-2);
    border-radius: var(--radius-sm);
    color: var(--text-faint);
  }
  .search input {
    flex: 1;
    border: none;
    outline: none;
    background: none;
    font-size: 12.5px;
  }
  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 0 8px 12px;
  }
  .row {
    display: block;
    width: 100%;
    text-align: left;
    padding: 9px 10px;
    border-radius: var(--radius);
    margin-bottom: 1px;
  }
  .row:hover {
    background: var(--panel-2);
  }
  .row.selected {
    background: var(--accent-soft);
  }
  .row-top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .title {
    font-weight: 550;
    font-size: 12.8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .slack {
    color: var(--text-faint);
    display: inline-flex;
  }
  .row-mid {
    color: var(--text-dim);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 1px 0 3px;
  }
  .row-bot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    color: var(--text-faint);
  }
  .row-bot .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    padding: 24px 12px;
    text-align: center;
    color: var(--text-faint);
  }
  .extras-toggle {
    display: block;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    margin-top: 10px;
    border-top: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 12px;
  }
  .extra {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
  }
  .extra:hover {
    background: var(--panel-2);
  }
  .extra .mono {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
