<script lang="ts">
  // A vertical drag handle on a pane's right edge. Reports the pointer's
  // horizontal movement since the drag started; the owner applies and clamps the width.
  let {
    onStart,
    onDrag,
    onEnd,
    onReset,
  }: {
    onStart: () => void;
    onDrag: (dx: number) => void;
    onEnd: () => void;
    onReset: () => void;
  } = $props();

  let startX: number | null = $state(null);

  function down(e: PointerEvent) {
    startX = e.clientX;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    onStart();
  }

  function move(e: PointerEvent) {
    if (startX !== null) onDrag(e.clientX - startX);
  }

  function up() {
    if (startX === null) return;
    startX = null;
    onEnd();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="handle"
  class:active={startX !== null}
  title="Drag to resize · double-click to reset"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={onReset}
></div>

<style>
  .handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 5;
    /* Inside the pane's edge: panes clip overflow, so it can't straddle the border. */
    right: 0;
  }
  .handle:hover,
  .handle.active {
    background: var(--accent-soft);
  }
</style>
