<!-- @component The list of all demos on the home page. -->
<script lang="ts">
  import { resolve } from '$app/paths'
  import { DEMOS } from '../../demos/index.ts'
  import IndexEntry from '../molecules/IndexEntry.svelte'
</script>

<ol aria-label="Demos">
  {#each DEMOS as demo (demo.route)}
    <li><IndexEntry href={resolve(demo.route)} name={demo.name} blurb={demo.blurb} rust={demo.rust} /></li>
  {/each}
</ol>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    border-bottom: var(--border-hairline) solid var(--color-ink-faint);
  }
</style>
