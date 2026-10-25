<!-- @component Two-column grid of readouts, ruled like a drawing's title block. -->
<script lang="ts">
  import Readout from '../atoms/Readout.svelte'
  import type { ReadoutItem } from '../types.ts'

  let { items }: { items: readonly ReadoutItem[] } = $props()
</script>

<dl>
  {#each items as item (item.label)}
    <Readout {...item} />
  {/each}
</dl>

<style>
  dl {
    display: grid;
    grid-template-columns: 1fr 1fr;
    margin: 0;
    border-bottom: var(--border-hairline) solid var(--color-rule);
  }

  dl > :global(:nth-child(odd)) {
    border-right: var(--border-hairline) solid var(--color-ink-faint);
  }

  dl > :global(:nth-child(n + 3)) {
    border-top: var(--border-hairline) solid var(--color-ink-faint);
  }
</style>
