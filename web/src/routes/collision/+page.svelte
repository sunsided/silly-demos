<script lang="ts">
  import { browser } from '$app/env'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import Select from '#lib/components/atoms/Select.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import CollisionCanvas from '#lib/components/organisms/CollisionCanvas.svelte'
  import MethodTable from '#lib/components/organisms/MethodTable.svelte'
  import SceneStats from '#lib/components/organisms/SceneStats.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import { CollisionLabState } from '#lib/demos/collision/lab.svelte.ts'
  import {
    BOUNDING_BOX,
    BOUNDING_CIRCLE,
    FULL_TEST,
    QUADTREE,
    STAGES,
    formatDistance,
    formatTime,
  } from '#lib/demos/collision/results.ts'
  import { SHAPE_KINDS, SHAPE_NAMES, isShapeKind } from '#lib/demos/collision/shapes.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  const lab = new CollisionLabState()
  const shapeOptions = SHAPE_KINDS.map(k => ({ value: k, label: SHAPE_NAMES[k] }))

  // Shapes and method live in the query string (`?a=obb&b=triangle&m=sat`) so a
  // case can be shared. Prerendered pages have no query, so only the browser reads it.
  if (browser) {
    const query = page.url.searchParams
    const [a, b] = [query.get('a'), query.get('b')]
    if (isShapeKind(a)) lab.setKind(0, a)
    if (isShapeKind(b)) lab.setKind(1, b)
    lab.wanted = query.get('m')
  }

  $effect(() => {
    const url = new URL(page.url.href)
    url.searchParams.set('a', lab.kinds[0])
    url.searchParams.set('b', lab.kinds[1])
    if (lab.method) url.searchParams.set('m', lab.method)
    if (url.search !== page.url.search) goto(url, { shallow: true, replace: true })
  })

  let selected = $derived(lab.selected)
  let counts = $derived(selected?.counts ?? [0, 0, 0, 0, 0, 0])
</script>

<SheetLayout
  title="Collision lab"
  summary="Pick two shapes, then switch between the methods that can test them. Each method sketches its own construction. Drag shapes and their handles."
  readouts={[
    {
      label: 'Intersecting',
      value: selected ? (selected.intersect ? 'Yes' : 'No') : '—',
      tone: selected?.intersect ? 'signal' : undefined,
    },
    {
      label: 'Distance',
      value: selected?.stage === FULL_TEST ? formatDistance(selected.distance) : '—',
      tone: selected?.distance !== undefined && selected.distance <= 0 ? 'signal' : undefined,
    },
    {
      label: 'Decided by',
      value: selected ? STAGES[selected.stage] : '—',
      tone: selected && selected.stage !== FULL_TEST ? 'cobalt' : undefined,
    },
    { label: 'Time per test', value: lab.method ? formatTime(lab.timings[lab.method]) : '—' },
    { label: 'Products', value: counts[0] },
    { label: 'Roots / divisions', value: `${counts[1]} / ${counts[2]}` },
    { label: 'Axes / supports', value: `${counts[3]} / ${counts[4]}` },
    { label: 'Iterations', value: counts[5] },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <CollisionCanvas {lab} {wasm} />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ControlGroup title="Shapes">
      <Select
        label="Shape A"
        bind:value={() => lab.kinds[0], kind => lab.setKind(0, kind)}
        options={shapeOptions}
      />
      <Select
        label="Shape B"
        bind:value={() => lab.kinds[1], kind => lab.setKind(1, kind)}
        options={shapeOptions}
      />
    </ControlGroup>

    {#if lab.method}
      <ControlGroup title="Method">
        <Choice
          name="method"
          bind:value={() => lab.method ?? '', id => (lab.wanted = id)}
          options={lab.ids.map(id => ({
            value: id,
            label: lab.methodName(id),
            detail: id === lab.method ? lab.methodSummary(id) : undefined,
          }))}
        />
      </ControlGroup>
    {/if}

    <ControlGroup title="Broadphase">
      <Toggle
        label="Bounding circles first"
        bind:checked={() => (lab.broadphase & BOUNDING_CIRCLE) !== 0, on => lab.setFlag(BOUNDING_CIRCLE, on)}
      />
      <Toggle
        label="Bounding boxes first"
        bind:checked={() => (lab.broadphase & BOUNDING_BOX) !== 0, on => lab.setFlag(BOUNDING_BOX, on)}
      />
      <Toggle
        label="Quadtree over the scene"
        bind:checked={() => lab.quadtreeOn, on => lab.setFlag(QUADTREE, on)}
      />
      {#if lab.quadtreeOn}
        <Slider label="Crowd" unit="shapes" bind:value={lab.crowdSetting} min={0} max={400} step={10} />
      {/if}
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    {#if lab.scene}
      <SceneStats stats={lab.scene} times={lab.sceneTimes} />
    {/if}
    <MethodTable
      rows={lab.rows}
      selected={lab.method}
      reference={lab.reference}
      timings={lab.timings}
      name={id => lab.methodName(id)}
      onselect={id => (lab.wanted = id)}
    />
  {/snippet}
</SheetLayout>
