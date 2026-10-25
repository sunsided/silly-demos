<!-- @component Inline color key. -->
<script lang="ts">
  import Swatch from '../atoms/Swatch.svelte'
  import type { Tone } from '../types.ts'

  let { items }: { items: readonly { tone: Tone; label: string }[] } = $props()
</script>

<ul>
  {#each items as item (item.label)}
    <li><Swatch tone={item.tone} />{item.label}</li>
  {/each}
</ul>

<style>
  ul {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2xs) var(--space-lg);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
  }
</style>
