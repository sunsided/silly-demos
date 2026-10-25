<!-- @component Scrolling page with a full-width banner on top. -->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { banner, children }: { banner: Snippet; children: Snippet } = $props()
</script>

<div class="landing">
  {@render banner()}
  {@render children()}
</div>

<style>
  .landing {
    height: 100%;
    overflow-y: auto;
  }

  @media (max-width: 900px) {
    .landing {
      overflow: visible;
    }
  }
</style>
