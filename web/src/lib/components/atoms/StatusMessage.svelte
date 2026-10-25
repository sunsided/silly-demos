<!-- @component A centered status line filling its container, e.g. while loading. -->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { children }: { children: Snippet } = $props()
</script>

<div class="status" role="status">{@render children()}</div>

<style>
  .status {
    display: grid;
    place-items: center;
    height: 100%;
    padding: var(--space-3xl);
    color: var(--color-ink-soft);
  }
</style>
