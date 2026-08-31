<script lang="ts">
  import type { InboxItem } from "./api";
  import { api } from "./api";
  import { toast, toastError } from "./toast.svelte";

  let {
    onClose,
    onCreated,
  }: {
    onClose: () => void;
    onCreated: (item: InboxItem) => void;
  } = $props();

  let title = $state("");
  let body = $state("");
  let saving = $state(false);
  let titleInput: HTMLInputElement | undefined = $state();

  $effect(() => titleInput?.focus());

  // Mirrors the Rust slugger for a live filename preview (Rust stays authoritative).
  const filenamePreview = $derived.by(() => {
    const slug = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .slice(0, 50)
      .replace(/^-+|-+$/g, "");
    if (!slug) return null;
    const date = new Date().toISOString().slice(0, 10);
    return `${date}-${slug}.md`;
  });

  async function create() {
    if (!title.trim() || saving) return;
    saving = true;
    try {
      const item = await api.createInboxItem(title, body);
      toast(`Created ${item.filename}`);
      onCreated(item);
    } catch (e) {
      toastError(e);
      saving = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
    if (e.metaKey && e.key === "Enter") void create();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="modal">
    <h3>New inbox item</h3>
    <input
      type="text"
      placeholder="Title"
      bind:value={title}
      bind:this={titleInput}
      onkeydown={(e) => e.key === "Enter" && create()}
    />
    <textarea placeholder="Details (optional, markdown)" bind:value={body} rows="7"></textarea>
    <div class="foot">
      <span class="mono">{filenamePreview ?? "inbox/…"}</span>
      <div class="btns">
        <button class="btn" onclick={onClose}>Cancel</button>
        <button class="btn primary" onclick={create} disabled={!title.trim() || saving}>
          Create
        </button>
      </div>
    </div>
  </div>
</div>

<style>
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
    width: 520px;
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
  input,
  textarea {
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
    padding: 8px 10px;
    font-size: 13px;
    outline: none;
    resize: vertical;
    user-select: text;
    cursor: text;
  }
  input:focus,
  textarea:focus {
    border-color: var(--accent);
  }
  textarea {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.55;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .foot .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btns {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
