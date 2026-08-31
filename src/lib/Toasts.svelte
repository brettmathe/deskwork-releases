<script lang="ts">
  import { toasts } from "./toast.svelte";
</script>

<div class="toasts">
  {#each toasts.list as t (t.id)}
    <div class="toast {t.kind}">{t.message}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    bottom: 16px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 100;
    max-width: 380px;
  }
  .toast {
    background: var(--panel);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 10px 14px;
    font-size: 12.5px;
    user-select: text;
  }
  .toast.error {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
