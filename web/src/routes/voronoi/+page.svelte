<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import VoronoiCanvas from '#lib/components/organisms/VoronoiCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import type { VoronoiStats } from '#lib/demos/voronoi/voronoi.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let count = $state(120)
  let speed = $state(12)
  let seed = $state(1337)
  let running = $state(true)
  let showDelaunay = $state(true)
  let showVoronoi = $state(true)
  let stats = $state<VoronoiStats>({ triangles: 0, edges: 0, hover: '–' })
</script>

<SheetLayout
  title="Voronoi & Delaunay"
  summary="The Delaunay triangulation connects points so that no point lies inside any triangle’s circumcircle. Joining the circumcenters of neighboring triangles gives the Voronoi diagram: each cell holds the area closest to one point. Hover to inspect a cell or triangle."
  readouts={[
    { label: 'Moving points', value: count },
    { label: 'Triangles', value: stats.triangles },
    { label: 'Voronoi edges', value: showVoronoi ? stats.edges : '–', tone: 'cobalt' },
    { label: 'Under pointer', value: stats.hover },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <VoronoiCanvas
          {wasm}
          {count}
          {speed}
          {seed}
          {running}
          {showDelaunay}
          {showVoronoi}
          onstats={s => (stats = s)}
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      <Button onclick={() => (seed = (seed * 1103515245 + 12345) >>> 0)}>New points</Button>
    </ButtonRow>
    <Toggle label="Delaunay triangulation" bind:checked={showDelaunay} />
    <Toggle label="Voronoi diagram" bind:checked={showVoronoi} />

    <ControlGroup title="Points">
      <Slider label="Count" bind:value={count} min={3} max={2000} step={1} />
      <Slider label="Speed" unit="px/s" bind:value={speed} min={0} max={200} />
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    <p>
      Show only the triangulation to inspect triangles and their circumcircles; with the Voronoi diagram shown,
      hovering picks the cell of the nearest point.
    </p>
  {/snippet}
</SheetLayout>
