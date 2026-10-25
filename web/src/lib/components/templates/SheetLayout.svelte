<!--
  @component
  Demo page layout: the drawing fills the sheet, the title block sits beside
  it (below it on narrow screens).
-->
<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte'
  import TitleBlock from '../organisms/TitleBlock.svelte'

  let { drawing, ...titleBlock }: ComponentProps<typeof TitleBlock> & { drawing: Snippet } = $props()
</script>

<svelte:head>
  <title>{titleBlock.title} · silly-demos</title>
</svelte:head>

<div class="sheet">
  <div class="drawing grid-paper">{@render drawing()}</div>
  <TitleBlock {...titleBlock} />
</div>

<style>
  .sheet {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--size-title-block);
    height: 100%;
    min-height: 0;
  }

  .drawing {
    position: relative;
    min-width: 0;
    min-height: 0;
  }

  /* Canvases fill the drawing area. */
  .drawing :global(canvas) {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: crosshair;
  }

  @media (max-width: 900px) {
    .sheet {
      grid-template-columns: 1fr;
      grid-template-rows: 62vh auto;
      height: auto;
    }
  }
</style>
