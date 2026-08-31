<script lang="ts">
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { InboxItemFull } from "./api";
  import { api } from "./api";
  import { renderMarkdown, handleMarkdownClick } from "./markdown";
  import { toast, toastError } from "./toast.svelte";
  import Icon from "./Icon.svelte";

  let {
    item,
    onChanged,
    onGone,
    onDirty,
  }: {
    item: InboxItemFull | null;
    onChanged: () => void;
    onGone: (filename: string) => void;
    onDirty: (dirty: boolean) => void;
  } = $props();

  let editing = $state(false);
  let draft = $state("");

  const dirty = $derived(editing && item !== null && draft !== item.content);
  $effect(() => onDirty(dirty));

  // Leaving edit mode when the selection changes is handled by the parent
  // (it guards on dirty); a plain selection change just resets the editor.
  $effect(() => {
    item?.filename;
    editing = false;
  });

  const slackUrl = $derived(
    item?.isSlack ? (item.content.match(/https:\/\/\S*slack\.com\/\S+/)?.[0] ?? null) : null,
  );

  function startEdit() {
    if (!item) return;
    draft = item.content;
    editing = true;
  }

  async function save() {
    if (!item || !editing) return;
    try {
      await api.saveInboxItem(item.filename, draft);
      editing = false;
      onChanged();
      toast("Saved");
    } catch (e) {
      toastError(e);
    }
  }

  function cancelEdit() {
    editing = false;
  }

  async function complete() {
    if (!item) return;
    try {
      await api.completeInboxItem(item.filename);
      toast(`Moved to completed/`);
      onGone(item.filename);
    } catch (e) {
      toastError(e);
    }
  }

  async function remove() {
    if (!item) return;
    const ok = await confirm(`Delete ${item.filename}?\n\nThis removes the file from inbox/.`, {
      title: "Delete item",
      kind: "warning",
    });
    if (!ok) return;
    try {
      await api.deleteInboxItem(item.filename);
      toast("Deleted");
      onGone(item.filename);
    } catch (e) {
      toastError(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (editing && e.metaKey && e.key === "s") {
      e.preventDefault();
      void save();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="pane">
  {#if item}
    <header data-tauri-drag-region>
      <div class="meta" data-tauri-drag-region>
        <span>{item.added ?? "no date"}</span>
        <span class="mono">{item.filename}</span>
      </div>
      <div class="actions">
        {#if editing}
          <button class="btn" onclick={cancelEdit}>Cancel</button>
          <button class="btn primary" onclick={save} disabled={!dirty} title="⌘S">
            <Icon name="check" size={13} />
            Save
          </button>
        {:else}
          {#if slackUrl}
            <button class="btn" onclick={() => openUrl(slackUrl).catch(toastError)}>
              <Icon name="slack" size={13} />
              Open in Slack
            </button>
          {/if}
          <button class="btn" onclick={startEdit}>
            <Icon name="pencil" size={13} />
            Edit
          </button>
          <button class="btn primary" onclick={complete}>
            <Icon name="check" size={13} />
            Complete
          </button>
          <button class="icon-btn danger" onclick={remove} title="Delete">
            <Icon name="trash" size={14} />
          </button>
        {/if}
      </div>
    </header>

    {#if editing}
      <textarea class="editor" bind:value={draft} spellcheck="false"></textarea>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
      <div class="content" onclick={(e) => handleMarkdownClick(e)}>
        <div class="md">{@html renderMarkdown(item.content)}</div>
      </div>
    {/if}
  {:else}
    <div class="empty-state" data-tauri-drag-region>
      <Icon name="inbox" size={36} />
      <span>Select an item to read it</span>
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
  .meta {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 12px;
    color: var(--text-dim);
    overflow: hidden;
  }
  .meta .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .icon-btn.danger:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 22px 26px 40px;
  }
  .editor {
    flex: 1;
    resize: none;
    border: none;
    outline: none;
    background: var(--panel);
    padding: 20px 26px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.6;
    user-select: text;
    cursor: text;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
