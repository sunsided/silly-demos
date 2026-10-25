<!-- @component What the quadtree stage did to the whole scene, and what it cost. -->
<script lang="ts">
  import { formatTime, percent, type SceneStats } from '../../demos/collision/results.ts'

  let {
    stats,
    times,
  }: { stats: SceneStats; times: { quadtree: number; brute: number } | null } = $props()
</script>

<p>
  <strong>Quadtree</strong> over {stats.bodies} bodies: {stats.nodes} cells, depth {stats.depth}.
  {stats.candidates.toLocaleString()} candidate pairs instead of {stats.allPairs.toLocaleString()}
  ({percent(stats.candidates, stats.allPairs)}); A and B have {stats.partners}. Of the candidates, {stats.boxOverlaps}
  have overlapping boxes and <strong>{stats.collisions} really collide</strong>
  ({percent(stats.collisions, stats.candidates)}).
  {#if times}
    Build and collect pairs {formatTime(times.quadtree)}, brute force {formatTime(times.brute)}.
  {/if}
</p>

<style>
  p {
    margin: 0 0 12px;
    font-variant-numeric: tabular-nums;
  }

  strong {
    color: var(--color-ink);
  }
</style>
