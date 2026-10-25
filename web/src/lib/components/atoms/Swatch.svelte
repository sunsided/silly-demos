<!-- @component Arrow-shaped color key, matching the boid chevrons. -->
<script lang="ts">
  import type { Tone } from '../types.ts'

  let { tone = 'ink' }: { tone?: Tone } = $props()
</script>

<span class="swatch" style:--tone="var(--color-{tone})"></span>

<style>
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    background: var(--tone);
    clip-path: polygon(100% 50%, 0 0, 30% 50%, 0 100%);
  }
</style>
