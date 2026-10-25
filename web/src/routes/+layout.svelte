<script lang="ts">
  import '#lib/tokens/generated/tokens.css'
  import '#lib/styles/global.css'
  import type { Snippet } from 'svelte'
  import Masthead from '#lib/components/organisms/Masthead.svelte'

  let { children }: { children: Snippet } = $props()
</script>

<div class="app">
  <Masthead />
  <main>{@render children()}</main>
</div>

<style>
  .app {
    display: grid;
    /* minmax keeps the scrolling nav from widening the page on small screens. */
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: var(--size-masthead) 1fr;
    height: 100%;
  }

  main {
    min-height: 0;
  }

  @media (max-width: 900px) {
    .app {
      grid-template-rows: auto 1fr;
      height: auto;
      min-height: 100%;
    }
  }
</style>
