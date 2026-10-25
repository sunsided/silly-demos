import type { Point } from '../../canvas/canvas.svelte.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'

export type HandleName = 'a' | 'b' | 'p'
export type Closest = 'interior' | 'a' | 'b'

export interface PointLineStats {
  distance: number
  perpendicular: number
  side: number
  closest: Closest
}

export const HIT_RADIUS = 14

/** Small square marking a right angle at `at`, between directions `u` and `v`. */
export function rightAngle(ctx: CanvasRenderingContext2D, at: Point, u: Point, v: Point, color: string, s = 9) {
  ctx.beginPath()
  ctx.moveTo(at.x + u.x * s, at.y + u.y * s)
  ctx.lineTo(at.x + (u.x + v.x) * s, at.y + (u.y + v.y) * s)
  ctx.lineTo(at.x + v.x * s, at.y + v.y * s)
  ctx.strokeStyle = color
  ctx.lineWidth = 1
  ctx.stroke()
}

export function sideColor(theme: Theme, side: number) {
  return side > 0 ? theme.verdigris : side < 0 ? theme.cobalt : theme.signal
}
