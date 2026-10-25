<!-- @component A titled group of related controls. -->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { title, children }: { title: string; children: Snippet } = $props()
</script>

<fieldset>
  <legend>{title}</legend>
  {@render children()}
</fieldset>

<style>
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    width: 100%;
    margin-bottom: var(--space-sm);
    padding: 0 0 var(--space-2xs);
    border-bottom: var(--border-hairline) solid var(--color-ink-faint);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-bold);
    font-stretch: var(--font-stretch-wide);
  }
</style>
