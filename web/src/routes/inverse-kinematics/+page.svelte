<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import IkCanvas from '#lib/components/organisms/IkCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import { SOLVERS, type IkStats, type SolverId } from '#lib/demos/ik/ik.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let solver = $state<SolverId>('fabrik')
  let segments = $state(6)
  let length = $state(45)
  let iterations = $state(10)
  let damping = $state(20)
  let smoothing = $state(80)
  let animate = $state(false)
  let avoidCrossing = $state(true)
  let floor = $state(true)
  let showReach = $state(true)
  let showAngles = $state(false)
  let straighten = $state(0)
  let stats = $state<IkStats>({ error: 0, iterations: 0, reachable: true, reach: 0, contact: false, rerouting: false })
</script>

<SheetLayout
  title="Inverse kinematics"
  summary="Given where the tip of a jointed arm should go, find the joint angles that put it there. Four iterative solvers each improve the current pose a little per step. Drag the target or the base."
  readouts={[
    { label: 'Error', value: `${stats.error.toFixed(1)} px`, tone: stats.reachable ? 'signal' : 'amber' },
    { label: 'Iterations', value: `${stats.iterations} / ${iterations}`, tone: 'cobalt' },
    {
      label: 'Target',
      value: !stats.reachable
        ? 'Too far'
        : stats.rerouting
          ? 'Rerouting'
          : stats.contact
            ? 'In contact'
            : 'Reachable',
      tone: stats.reachable && !stats.contact && !stats.rerouting ? 'ink' : 'amber',
    },
    { label: 'Reach', value: `${Math.round(stats.reach)} px` },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <IkCanvas
          {wasm}
          {solver}
          {segments}
          {length}
          {iterations}
          {damping}
          {smoothing}
          {animate}
          {avoidCrossing}
          {floor}
          {showReach}
          {showAngles}
          {straighten}
          onstats={s => (stats = s)}
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (animate = !animate)}>{animate ? 'Stop target' : 'Animate target'}</Button>
      <Button onclick={() => straighten++}>Straighten</Button>
    </ButtonRow>
    <Toggle label="No self-crossing" bind:checked={avoidCrossing} />
    <Toggle label="Solid floor" bind:checked={floor} />
    <Toggle label="Reach circle" bind:checked={showReach} />
    <Toggle label="Joint angles" bind:checked={showAngles} />

    <ControlGroup title="Solver">
      <Choice
        name="solver"
        bind:value={solver}
        options={SOLVERS.map(s => ({ ...s, detail: s.value === solver ? s.detail : undefined }))}
      />
      {#if solver === 'dls'}
        <Slider label="Damping λ" unit="px" bind:value={damping} min={1} max={100} />
      {/if}
      <Slider label="Iterations per frame" bind:value={iterations} min={1} max={100} />
    </ControlGroup>

    <ControlGroup title="Chain">
      <Slider label="Segments" bind:value={segments} min={2} max={12} />
      <Slider label="Segment length" unit="px" bind:value={length} min={15} max={80} />
      <Slider label="Smoothing" unit="ms" bind:value={smoothing} min={0} max={300} step={10} />
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    <p>
      Set the iterations per frame to 1 and press Straighten to watch a solver converge step by step. Each solve starts
      from the last pose, so a slowly moving target needs only an iteration or two.
    </p>
    <p>
      The solvers know nothing about the arm’s body and will fold it through itself or sweep it through the ground.
      With self-crossing and the floor off limits, a step that brings two links too close, or a joint below the ground,
      is cut back by blending the joint angles towards the previous pose, so the arm slides along the obstacle. When
      that gets it stuck, it swings over to an evenly curled arc with its tip on the target, going via straight up if
      the direct way is blocked, and the solver carries on from there.
    </p>
  {/snippet}
</SheetLayout>
