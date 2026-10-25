<script lang="ts">
  import DemoIndex from '#lib/components/organisms/DemoIndex.svelte'
  import Hero from '#lib/components/organisms/Hero.svelte'
  import LandingLayout from '#lib/components/templates/LandingLayout.svelte'
</script>

<svelte:head>
  <title>silly-demos</title>
  <meta
    name="description"
    content="Small interactive geometry and simulation demos in Rust/WebAssembly, drawn as engineering sketches in the browser."
  />
</svelte:head>

<LandingLayout>
  {#snippet banner()}<Hero />{/snippet}
  <DemoIndex />
</LandingLayout>
