import { cross, handle, label } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'
import { tokens } from '../../tokens/generated/tokens.ts'
import { handles, type ShapeState } from './shapes.ts'

/** Path along a flat closed outline `[x0, y0, x1, y1, ...]`. */
export function tracePath(ctx: CanvasRenderingContext2D, outline: Float32Array) {
  ctx.beginPath()
  ctx.moveTo(outline[0], outline[1])
  for (let i = 2; i < outline.length; i += 2) ctx.lineTo(outline[i], outline[i + 1])
  ctx.closePath()
}

/** A shape with its outline, center mark and name; signal-colored when hit. */
export function drawShape(ctx: CanvasRenderingContext2D, t: Theme, outline: Float32Array, s: ShapeState, hit: boolean, name: string) {
  if (outline.length < 4) return
  tracePath(ctx, outline)
  ctx.fillStyle = hit ? t.signalFaint : t.cobaltFaint
  ctx.fill()
  ctx.lineWidth = 2
  ctx.lineJoin = 'round'
  ctx.strokeStyle = hit ? t.signal : t.ink
  ctx.stroke()
  if (s.kind === 'capsule') {
    // Core segment, the capsule's skeleton.
    const [p0, p1] = s.pts
    ctx.save()
    ctx.setLineDash([2, 4])
    ctx.lineWidth = 1
    ctx.beginPath()
    ctx.moveTo(s.c.x + p0.x, s.c.y + p0.y)
    ctx.lineTo(s.c.x + p1.x, s.c.y + p1.y)
    ctx.stroke()
    ctx.restore()
  }
  cross(ctx, s.c, hit ? t.signal : t.ink)
  let top = Infinity
  for (let i = 1; i < outline.length; i += 2) top = Math.min(top, outline[i])
  label(ctx, t, name, { x: s.c.x + 10, y: top - 12 }, hit ? t.signal : t.ink, 'left')
}

export function drawHandles(ctx: CanvasRenderingContext2D, t: Theme, s: ShapeState) {
  for (const h of handles(s)) {
    if (h.guide) {
      ctx.save()
      ctx.setLineDash([2, 4])
      ctx.strokeStyle = t.inkSoft
      ctx.beginPath()
      ctx.moveTo(s.c.x, s.c.y)
      ctx.lineTo(h.at.x, h.at.y)
      ctx.stroke()
      ctx.restore()
    }
    handle(ctx, h.at, t.ink, t.paper, tokens.size.handle)
  }
}

/** Crowd outlines, as returned by `CollisionLab.crowd_outlines()`. */
export function drawCrowd(ctx: CanvasRenderingContext2D, t: Theme, buf: Float32Array) {
  // Ghosted: only the quadtree's notes pick out the bodies that matter to A and B.
  ctx.save()
  ctx.strokeStyle = t.inkFaint
  ctx.lineWidth = 1
  for (let i = 0; i < buf.length;) {
    const n = buf[i]
    tracePath(ctx, buf.subarray(i + 1, i + 1 + n * 2))
    ctx.stroke()
    i += 1 + n * 2
  }
  ctx.restore()
}
