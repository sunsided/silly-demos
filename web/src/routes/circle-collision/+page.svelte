<!--
  The circle demo became the collision lab; old links land on its circle/circle
  case. A meta refresh rather than `redirect()`, so prerendering doesn't follow
  the query-string URL and write it out as a page of its own.
-->
<script lang="ts">
  import { resolve } from '$app/paths'

  const target = `${resolve('/collision')}?a=circle&b=circle`
</script>

<svelte:head>
  <meta http-equiv="refresh" content="0;url={target}" />
  <link rel="canonical" href={target} />
</svelte:head>

<p>The circle collision demo is now part of the <a href={target}>collision lab</a>.</p>
