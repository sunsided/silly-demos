<script lang="ts">
  import PointLineCanvas from '#lib/components/organisms/PointLineCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import type { Tone } from '#lib/components/types.ts'
  import type { PointLineStats } from '#lib/demos/point-line/point-line.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let stats = $state<PointLineStats>({ distance: 0, perpendicular: 0, side: 0, closest: 'interior' })

  let sideTone = $derived<Tone>(stats.side > 0 ? 'verdigris' : stats.side < 0 ? 'cobalt' : 'signal')
  let sideText = $derived(stats.side > 0 ? 'Right' : stats.side < 0 ? 'Left' : 'On the line')
  let closestText = $derived({ interior: 'Between A, B', a: 'End A', b: 'End B' }[stats.closest])
</script>

<SheetLayout
  title="Point to segment"
  summary="The shortest distance from P to the segment AB, and how it differs from the perpendicular distance to the infinite line. Drag A, B or P."
  readouts={[
    { label: 'Distance', value: stats.distance.toFixed(1), tone: sideTone },
    { label: 'Perpendicular', value: stats.perpendicular.toFixed(1) },
    { label: 'Side of A→B', value: sideText, tone: sideTone },
    { label: 'Closest at', value: closestText },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <PointLineCanvas {wasm} onstats={s => (stats = s)} />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet notes()}
    <p>
      When P projects past an end point, the nearest point is that end point, and the dotted perpendicular to the
      infinite line is shorter than the true distance.
    </p>
  {/snippet}
</SheetLayout>
