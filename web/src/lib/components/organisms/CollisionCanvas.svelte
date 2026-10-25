<!--
  @component
  The collision lab's drawing: two draggable shapes, the selected method's
  construction and, with the quadtree stage on, a drifting crowd.
-->
<script lang="ts">
  import { prefersReducedMotion } from 'svelte/motion'
  import { DrawingCanvas, draggable, onFrame } from '../../canvas/canvas.svelte.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import type { CollisionLabState } from '../../demos/collision/lab.svelte.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let { lab, wasm }: { lab: CollisionLabState; wasm: Wasm } = $props()

  $effect(() => lab.connect(wasm))

  const canvas = new DrawingCanvas(size => lab.fit(size))
  const drag = draggable({
    toLocal: canvas.centered,
    pick: p => lab.pick(p),
    move: (handle, delta) => lab.move(handle, delta),
  })

  // The crowd is rebuilt when its size changes and keeps drifting afterwards.
  $effect(() => {
    const count = lab.crowd
    lab.resetCrowd(count, canvas.size.width || 800, canvas.size.height || 600)
  })

  // The scene keeps moving, so the quadtree is re-timed on an interval rather than on settle.
  $effect(() => {
    if (!lab.quadtreeOn) return
    void lab.crowd
    const first = setTimeout(() => lab.measureScene(), 300)
    const every = setInterval(() => lab.measureScene(), 1500)
    return () => {
      clearTimeout(first)
      clearInterval(every)
      lab.sceneTimes = null
    }
  })

  onFrame((_, dt) => {
    const ctx = canvas.context()
    if (ctx) lab.frame(ctx, canvas.size, currentTheme(), dt, prefersReducedMotion.current)
  })
</script>

<canvas {@attach canvas.attach} {@attach drag}>Two draggable shapes with the selected collision method's construction</canvas>
