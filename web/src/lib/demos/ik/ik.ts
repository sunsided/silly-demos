import type { Point } from '../../canvas/canvas.svelte.ts'
import { hatchPattern, label } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'

export type SolverId = 'fabrik' | 'ccd' | 'jacobian-transpose' | 'dls'
export type HandleName = 'base' | 'target'

/** The solvers in menu order; ids match `IkChain::solvers` on the Rust side. */
export const SOLVERS: readonly { value: SolverId; label: string; detail: string }[] = [
  {
    value: 'fabrik',
    label: 'FABRIK',
    detail:
      'Drags the chain from the target back to the base and out again, re-stretching each segment. Moves joint positions, never angles.',
  },
  {
    value: 'ccd',
    label: 'Cyclic coordinate descent',
    detail: 'Turns one joint at a time, from the tip inwards, so the end effector points at the target.',
  },
  {
    value: 'jacobian-transpose',
    label: 'Jacobian transpose',
    detail:
      'Steps all joint angles down the gradient of the distance to the target. Cheap per step, but slow to settle, and stuck when the arm points straight away from the target.',
  },
  {
    value: 'dls',
    label: 'Damped least squares',
    detail:
      'Solves for the angle change that best moves the end effector onto the target, damped by λ so stretched, nearly singular poses don’t make it jump.',
  },
]

export interface IkStats {
  error: number
  iterations: number
  reachable: boolean
  reach: number
  /** A step was cut short to keep the links apart. */
  contact: boolean
  /** The arm got stuck against itself and is swinging round. */
  rerouting: boolean
}

export const HIT_RADIUS = 14

const LINK_WIDTH = 14

/** Closest the link center lines may come with self-crossing avoided: outlines never touch. */
export const CLEARANCE = LINK_WIDTH + 2

/** Height of the mount from the base pivot down to the ground line. */
const MOUNT_HEIGHT = 18

/** How far below the base the joints may go with the floor on: link outlines stop at the ground line. */
export const FLOOR = MOUNT_HEIGHT - LINK_WIDTH / 2

/** Joint `i` of a flat joint buffer. */
export const joint = (joints: Float32Array, i: number): Point => ({ x: joints[i * 2], y: joints[i * 2 + 1] })

/**
 * The fixed mount: a triangular foot on a hatched ground strip reaching
 * `ground` px to either side, so a solid floor can span the drawing.
 */
export function drawBase(ctx: CanvasRenderingContext2D, t: Theme, base: Point, dpr: number, ground = 40) {
  const floor = base.y + MOUNT_HEIGHT
  ctx.save()
  ctx.beginPath()
  ctx.rect(base.x - ground + 6, floor, (ground - 6) * 2, 10)
  ctx.fillStyle = hatchPattern(ctx, t.inkSoft, dpr)
  ctx.fill()
  ctx.lineWidth = 1.5
  ctx.strokeStyle = t.ink
  ctx.beginPath()
  ctx.moveTo(base.x - ground, floor)
  ctx.lineTo(base.x + ground, floor)
  ctx.stroke()
  ctx.beginPath()
  ctx.moveTo(base.x, base.y)
  ctx.lineTo(base.x - 16, floor)
  ctx.lineTo(base.x + 16, floor)
  ctx.closePath()
  ctx.fillStyle = t.paper
  ctx.fill()
  ctx.stroke()
  ctx.restore()
}

/** The links as outlined bars with a dashed center line, and a pin at every joint. */
export function drawChain(ctx: CanvasRenderingContext2D, t: Theme, joints: Float32Array) {
  const n = joints.length / 2
  ctx.save()
  ctx.lineCap = 'round'
  // Outline then fill per link, so each link sits on top of the one before.
  for (let i = 0; i + 1 < n; i++) {
    const a = joint(joints, i)
    const b = joint(joints, i + 1)
    ctx.beginPath()
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
    ctx.lineWidth = LINK_WIDTH
    ctx.strokeStyle = t.ink
    ctx.stroke()
    ctx.lineWidth = LINK_WIDTH - 3
    ctx.strokeStyle = t.paper
    ctx.stroke()
  }
  ctx.beginPath()
  for (let i = 0; i < n; i++) {
    const p = joint(joints, i)
    if (i === 0) ctx.moveTo(p.x, p.y)
    else ctx.lineTo(p.x, p.y)
  }
  ctx.setLineDash([10, 3, 2, 3])
  ctx.lineWidth = 1
  ctx.strokeStyle = t.inkFaint
  ctx.stroke()
  ctx.setLineDash([])
  for (let i = 0; i + 1 < n; i++) {
    const p = joint(joints, i)
    ctx.beginPath()
    ctx.arc(p.x, p.y, 3.5, 0, Math.PI * 2)
    ctx.fillStyle = t.paper
    ctx.fill()
    ctx.lineWidth = 1.5
    ctx.strokeStyle = t.ink
    ctx.stroke()
  }
  ctx.restore()
}

/**
 * The bend at every inner joint: an arc from the previous link's extension to
 * the next link, labelled in degrees (counter-clockwise on screen positive).
 */
export function drawJointAngles(ctx: CanvasRenderingContext2D, t: Theme, joints: Float32Array) {
  const n = joints.length / 2
  for (let i = 1; i + 1 < n; i++) {
    const prev = joint(joints, i - 1)
    const p = joint(joints, i)
    const next = joint(joints, i + 1)
    const from = Math.atan2(p.y - prev.y, p.x - prev.x)
    const to = Math.atan2(next.y - p.y, next.x - p.x)
    let bend = to - from
    bend = Math.atan2(Math.sin(bend), Math.cos(bend))
    if (Math.abs(bend) < 0.02) continue
    const r = 22
    ctx.save()
    ctx.beginPath()
    ctx.moveTo(p.x, p.y)
    ctx.lineTo(p.x + Math.cos(from) * (r + 6), p.y + Math.sin(from) * (r + 6))
    ctx.setLineDash([2, 3])
    ctx.lineWidth = 1
    ctx.strokeStyle = t.inkSoft
    ctx.stroke()
    ctx.setLineDash([])
    ctx.beginPath()
    ctx.arc(p.x, p.y, r, from, to, bend < 0)
    ctx.strokeStyle = t.cobalt
    ctx.stroke()
    ctx.restore()
    const mid = from + bend / 2
    // Canvas y points down, so a positive screen angle is clockwise; flip it.
    label(ctx, t, `${Math.round((-bend * 180) / Math.PI)}°`, {
      x: p.x + Math.cos(mid) * (r + 14),
      y: p.y + Math.sin(mid) * (r + 14),
    }, t.cobalt)
  }
}
