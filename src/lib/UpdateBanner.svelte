<script lang="ts">
  import Icon from "./Icon.svelte";
  import { installUpdate, restartApp, updater } from "./updater.svelte";

  // Only the states that warrant interrupting the sidebar.
  const shown = $derived(
    updater.status === "available" ||
      updater.status === "downloading" ||
      updater.status === "ready",
  );
  const pct = $derived(updater.progress === null ? null : Math.round(updater.progress * 100));
</script>

{#if shown}
  <div class="update" role="status">
    <div class="row">
      <Icon name="download" size={13} />
      {#if updater.status === "ready"}
        <span class="label">Update installed</span>
      {:else}
        <span class="label">Version {updater.version}</span>
      {/if}
    </div>

    {#if updater.status === "available"}
      <button class="act" onclick={() => void installUpdate()}>Install</button>
    {:else if updater.status === "downloading"}
      <div class="bar" aria-label="Downloading update">
        <div class="fill" class:indeterminate={pct === null} style={pct === null ? "" : `width:${pct}%`}></div>
      </div>
      <span class="sub">{pct === null ? "Downloading…" : `Downloading ${pct}%`}</span>
    {:else}
      <button class="act" onclick={() => void restartApp()}>Restart to finish</button>
    {/if}
  </div>
{/if}

<style>
  .update {
    margin: 0 10px 8px;
    padding: 9px 10px;
    border: 1px solid var(--accent);
    background: var(--accent-soft);
    border-radius: var(--radius-sm);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--accent);
  }
  .label {
    font-size: 11.5px;
    font-weight: 600;
  }
  .sub {
    display: block;
    font-size: 10.5px;
    color: var(--text-dim);
    margin-top: 4px;
  }
  .act {
    display: block;
    width: 100%;
    margin-top: 7px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: #fff;
    font-size: 11.5px;
    font-weight: 600;
  }
  @media (prefers-color-scheme: dark) {
    .act {
      color: #0f1211;
    }
  }
  .act:hover {
    background: var(--accent-hover);
  }
  .bar {
    height: 4px;
    margin-top: 8px;
    border-radius: 2px;
    background: var(--border-strong);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  /* No content-length from the server — show motion instead of a false number. */
  .fill.indeterminate {
    width: 40%;
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes slide {
    0% { margin-left: -40%; }
    100% { margin-left: 100%; }
  }
</style>
