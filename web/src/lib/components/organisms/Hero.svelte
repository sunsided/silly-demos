<!-- @component Home page banner: the site title over a drifting flock (still under reduced motion). -->
<script lang="ts">
  import { prefersReducedMotion } from 'svelte/motion'
  import { DEFAULT_FLOCK } from '../../demos/boids/flock.ts'
  import WasmGate from '../../wasm/WasmGate.svelte'
  import FlockCanvas from './FlockCanvas.svelte'

  const flock = { ...DEFAULT_FLOCK, count: 260 }
</script>

<section class="hero grid-paper">
  <div class="flock">
    <WasmGate quiet>
      {#snippet children(wasm)}
        <FlockCanvas
          {wasm}
          config={flock}
          running={!prefersReducedMotion.current}
          label="A flock of boids drifting across the page"
        />
      {/snippet}
    </WasmGate>
  </div>
  <div class="text">
    <h1>silly-demos</h1>
    <p>
      Small geometry and simulation sketches. The math runs in Rust compiled to WebAssembly; the browser only
      draws.
    </p>
  </div>
</section>

<style>
  .hero {
    position: relative;
    display: flex;
    align-items: flex-end;
    min-height: min(62vh, 560px);
    overflow: hidden;
    border-bottom: var(--border-hairline) solid var(--color-rule);
  }

  .flock {
    position: absolute;
    inset: 0;
  }

  .flock :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .text {
    position: relative;
    padding: 0 var(--space-3xl) 28px;
    pointer-events: none;
  }

  h1 {
    margin: 0;
    font-size: clamp(56px, 12vw, 168px);
    line-height: 0.86;
    font-weight: 850;
    font-stretch: var(--font-stretch-wider);
    letter-spacing: -0.045em;
    /* Paper-colored outline keeps the title readable as boids fly under it. */
    paint-order: stroke fill;
    -webkit-text-stroke: 10px var(--color-paper);
  }

  p {
    max-width: 46ch;
    margin: var(--space-xl) 0 0;
    font-size: var(--font-size-lg);
    line-height: 1.45;
    text-shadow:
      0 0 6px var(--color-paper),
      0 0 2px var(--color-paper);
  }

  @media (max-width: 900px) {
    .text {
      padding: 0 var(--space-lg) var(--space-2xl);
    }
  }
</style>
