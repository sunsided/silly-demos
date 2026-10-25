<!--
  @component
  The side panel of a demo sheet, laid out like a drawing's title block:
  name and summary, live readouts, controls and notes.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import ReadoutGrid from '../molecules/ReadoutGrid.svelte'
  import type { ReadoutItem } from '../types.ts'

  let {
    title,
    summary,
    readouts,
    controls,
    notes,
  }: {
    title: string
    summary: Snippet | string
    readouts: readonly ReadoutItem[]
    controls?: Snippet
    notes?: Snippet
  } = $props()
</script>

<aside aria-label="{title} details">
  <header>
    <h1>{title}</h1>
    <p>
      {#if typeof summary === 'string'}{summary}{:else}{@render summary()}{/if}
    </p>
  </header>
  <ReadoutGrid items={readouts} />
  {#if controls}
    <div class="controls">{@render controls()}</div>
  {/if}
  {#if notes}
    <div class="notes">{@render notes()}</div>
  {/if}
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
    border-left: var(--border-hairline) solid var(--color-rule);
    background: var(--color-paper-raised);
  }

  header {
    padding: var(--space-2xl) var(--space-2xl) var(--space-xl);
    border-bottom: var(--border-hairline) solid var(--color-rule);
  }

  h1 {
    margin: 0 0 var(--space-sm);
    font-size: var(--font-size-3xl);
    line-height: 1.05;
    font-weight: var(--font-weight-heavy);
    font-stretch: var(--font-stretch-wider);
    letter-spacing: -0.025em;
  }

  p {
    max-width: 60ch;
    margin: 0;
    color: var(--color-ink-soft);
    font-size: var(--font-size-md);
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: var(--space-xl);
    padding: var(--space-xl) var(--space-2xl) var(--space-2xl);
  }

  .notes {
    margin-top: auto;
    padding: var(--space-lg) var(--space-2xl) var(--space-xl);
    border-top: var(--border-hairline) solid var(--color-ink-faint);
    color: var(--color-ink-soft);
    font-size: var(--font-size-sm);
  }

  .notes :global(p) {
    margin: 0;
  }

  .notes :global(* + p) {
    margin-top: var(--space-xs);
  }

  @media (max-width: 900px) {
    aside {
      border-left: 0;
      border-top: var(--border-hairline) solid var(--color-rule);
      overflow: visible;
    }
  }
</style>
