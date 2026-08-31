<script lang="ts">
  import { icons, type IconName } from "./icons";

  let { name, size = 14 }: { name: IconName; size?: number } = $props();
</script>

<span class="icon" style="width:{size}px;height:{size}px">{@html icons[name]}</span>

<style>
  .icon {
    display: inline-flex;
    flex-shrink: 0;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
