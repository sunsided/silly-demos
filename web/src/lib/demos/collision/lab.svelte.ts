import type { CanvasSize, Point } from '../../canvas/canvas.svelte.ts'
import { clear, hatchPattern } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'
import { tokens } from '../../tokens/generated/tokens.ts'
import type { Wasm } from '../../wasm/wasm.ts'
import { drawCrowd, drawHandles, drawShape, tracePath } from './drawing.ts'
import { drawNotes } from './notes.ts'
import {
  QUADTREE,
  REFERENCE,
  readScene,
  sameRows,
  sameScene,
  timePerRun,
  type Row,
  type SceneStats,
} from './results.ts'
import { defaultShape, handles, insideOutline, params, translate, type ShapeKind, type ShapeState } from './shapes.ts'

type Lab = InstanceType<Wasm['CollisionLab']>

interface Drag {
  slot: number
  /** Index into `handles(shape)`, or null to move the whole shape. */
  handle: number | null
  pos: Point
}

/**
 * State and frame logic of the collision lab. The `$state` fields are what
 * the page shows and edits; everything the frame loop touches every frame is
 * a plain field.
 */
export class CollisionLabState {
  // Inputs.
  kinds = $state<[ShapeKind, ShapeKind]>(['circle', 'obb'])
  /** The method asked for (URL or click); falls back to the first that applies. */
  wanted = $state<string | null>(null)
  broadphase = $state(0)
  crowdSetting = $state(150)

  // Results.
  wasm = $state.raw<Wasm | null>(null)
  rows = $state.raw<Row[]>([])
  timings = $state.raw<Record<string, number>>({})
  scene = $state.raw<SceneStats | null>(null)
  sceneTimes = $state.raw<{ quadtree: number; brute: number } | null>(null)

  ids = $derived(this.rows.map(r => r.id))
  method = $derived(this.wanted && this.ids.includes(this.wanted) ? this.wanted : this.ids[0])
  selected = $derived(this.rows.find(r => r.id === this.method))
  reference = $derived(this.rows.find(r => r.id === REFERENCE))
  quadtreeOn = $derived((this.broadphase & QUADTREE) !== 0)
  /** The crowd only exists for the quadtree stage; without it there is nothing to prune. */
  crowd = $derived(this.quadtreeOn ? this.crowdSetting : 0)

  #lab: Lab | null = null
  #shapes: ShapeState[] = [defaultShape('circle', { x: -140, y: 0 }), defaultShape('obb', { x: 140, y: 0 })]
  #outlines: Float32Array[] = [new Float32Array(), new Float32Array()]
  #notes: { buf: Float32Array; labels: string[] } = { buf: new Float32Array(), labels: [] }
  /** Bumped on every shape edit; the frame loop only recomputes when it changed. */
  #version = 0
  /** Key and rows of the last computed state, redrawn as-is while nothing changes. */
  #computed: { key: string; rows: Row[] } | null = null
  #pose = ''
  #benchTimer = 0
  #scale = 1
  #placed = false

  /** Replaces shape `slot` with a fresh one of `kind`, at the same place. */
  setKind(slot: 0 | 1, kind: ShapeKind) {
    if (this.kinds[slot] === kind) return
    this.kinds[slot] = kind
    this.#shapes[slot] = defaultShape(kind, this.#shapes[slot].c, this.#scale)
    this.#version++
  }

  setFlag(flag: number, on: boolean) {
    this.broadphase = on ? this.broadphase | flag : this.broadphase & ~flag
  }

  methodName(id: string) {
    return this.wasm?.CollisionLab.method_name(id) ?? id
  }

  methodSummary(id: string) {
    return this.wasm?.CollisionLab.method_summary(id) ?? ''
  }

  /** Creates the WASM lab; returns the cleanup that frees it. */
  connect(wasm: Wasm) {
    const lab = new wasm.CollisionLab()
    this.#lab = lab
    this.#computed = null
    this.wasm = wasm
    return () => {
      clearTimeout(this.#benchTimer)
      this.#lab = null
      lab.free()
    }
  }

  /** Fits the initial layout to narrow screens, once. */
  fit({ width }: CanvasSize) {
    // A canvas laid out at zero width (e.g. hidden) would scale everything to the origin.
    if (this.#placed || width === 0) return
    this.#placed = true
    const k = Math.min(1, width / 700)
    if (k === 1) return
    this.#scale = k
    this.#shapes = this.#shapes.map(s => defaultShape(s.kind, { x: s.c.x * k, y: s.c.y * k }, k))
    this.#version++
  }

  /** Rebuilds the crowd inside a `width` × `height` area. */
  resetCrowd(count: number, width: number, height: number) {
    this.#lab?.set_crowd(count, width, height, 0x5eed)
  }

  /** Times the quadtree build against brute-force pair tests. */
  measureScene() {
    const lab = this.#lab
    if (!lab) return
    this.sceneTimes = {
      quadtree: timePerRun(n => lab.bench_quadtree(n)),
      brute: timePerRun(n => lab.bench_brute_force(n)),
    }
  }

  // Dragging, in coordinates centered on the canvas.

  pick = (p: Point): Drag | null => {
    // Handles win over bodies; B is drawn on top, so it wins ties.
    for (const slot of [1, 0]) {
      const hs = handles(this.#shapes[slot])
      const i = hs.findIndex(h => Math.hypot(h.at.x - p.x, h.at.y - p.y) <= tokens.size.handleHit)
      if (i >= 0) return { slot, handle: i, pos: hs[i].at }
    }
    for (const slot of [1, 0]) {
      if (insideOutline(this.#outlines[slot], p)) return { slot, handle: null, pos: p }
    }
    return null
  }

  move = (drag: Drag, d: Point) => {
    const s = this.#shapes[drag.slot]
    this.#version++
    if (drag.handle === null) return translate(s, d)
    drag.pos = { x: drag.pos.x + d.x, y: drag.pos.y + d.y }
    // The shape kind can change mid-drag (keyboard on the select); its handles change with it.
    handles(s)[drag.handle]?.set(s, drag.pos)
  }

  /** Steps the crowd, recomputes results if anything changed, and draws. */
  frame(ctx: CanvasRenderingContext2D, size: CanvasSize, theme: Theme, dt: number, still: boolean) {
    const lab = this.#lab
    if (!lab) return
    const [a, b] = this.#shapes
    const { broadphase, crowd, method } = this

    // A drifting crowd changes the scene every frame, so it always recomputes.
    const crowdMoving = crowd > 0 && !still
    if (crowdMoving) lab.step_crowd(Math.min(dt, 0.1))

    // Otherwise recompute only when the shapes, broadphase, crowd or selected method changed.
    const poseKey = `${this.#version}|${broadphase}`
    const key = `${poseKey}|${crowd}|${method}`
    if (crowdMoving || this.#computed?.key !== key) {
      lab.set_shape(0, a.kind, params(a))
      lab.set_shape(1, b.kind, params(b))
      this.#outlines = [lab.outline(0), lab.outline(1)]

      const rows: Row[] = []
      let scene: SceneStats | null = null
      for (const id of lab.methods()) {
        const r = lab.run(id, broadphase)
        if (!r) continue
        rows.push({ id, intersect: r.intersect, distance: r.distance, stage: r.stage, counts: Array.from(r.counts()) })
        if (id === method || (!method && rows.length === 1)) {
          this.#notes = { buf: r.notes(), labels: r.labels() }
          scene = readScene(r.scene())
        }
        r.free()
      }
      this.#computed = { key, rows }
      if (!sameRows(this.rows, rows)) this.rows = rows
      if (!sameScene(this.scene, scene)) this.scene = scene

      if (poseKey !== this.#pose) {
        this.#pose = poseKey
        this.#scheduleBench(rows.map(r => r.id), broadphase)
      }
    }
    const rows = this.#computed.rows

    clear(ctx, size.dpr)
    ctx.translate(size.width / 2, size.height / 2)

    const overlap = rows.find(r => r.id === REFERENCE)?.intersect ?? false
    const hit = (rows.find(r => r.id === method) ?? rows[0])?.intersect ?? false
    if (crowd > 0) drawCrowd(ctx, theme, lab.crowd_outlines())
    drawShape(ctx, theme, this.#outlines[0], a, hit, 'A')
    drawShape(ctx, theme, this.#outlines[1], b, hit, 'B')

    if (overlap) {
      // Hatch the true intersection, whatever the selected method claims.
      ctx.save()
      tracePath(ctx, this.#outlines[0])
      ctx.clip()
      tracePath(ctx, this.#outlines[1])
      ctx.fillStyle = hatchPattern(ctx, theme.signal, size.dpr)
      ctx.fill()
      ctx.restore()
    }

    drawNotes(ctx, theme, this.#notes.buf, this.#notes.labels)
    drawHandles(ctx, theme, a)
    drawHandles(ctx, theme, b)
  }

  /** Timing waits until the pose has settled, then runs every method in a tight loop. */
  #scheduleBench(methods: string[], mask: number) {
    clearTimeout(this.#benchTimer)
    if (Object.keys(this.timings).length) this.timings = {}
    this.#benchTimer = window.setTimeout(() => {
      const lab = this.#lab
      if (!lab) return
      const result: Record<string, number> = {}
      for (const id of methods) result[id] = timePerRun(n => lab.bench(id, mask, n))
      this.timings = result
    }, 250)
  }
}
