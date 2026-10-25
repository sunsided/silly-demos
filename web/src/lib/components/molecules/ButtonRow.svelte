<!-- @component A wrapping row of buttons. -->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { children }: { children: Snippet } = $props()
</script>

<div>{@render children()}</div>

<style>
  div {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-sm);
  }
</style>
