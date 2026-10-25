import type { Point } from './canvas'
import type { Theme } from './theme'

/** Drafting-style drawing helpers shared by the demos. */

export function clear(ctx: CanvasRenderingContext2D, dpr: number) {
  ctx.setTransform(1, 0, 0, 1, 0, 0)
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
}

/** Text with a paper-colored halo so it stays legible over lines. */
export function label(
  ctx: CanvasRenderingContext2D,
  theme: Theme,
  text: string,
  at: Point,
  color = theme.ink,
  align: CanvasTextAlign = 'center',
) {
  ctx.save()
  ctx.font = `500 12px ${theme.font}`
  ctx.textAlign = align
  ctx.textBaseline = 'middle'
  ctx.lineJoin = 'round'
  ctx.lineWidth = 4
  ctx.strokeStyle = theme.paper
  ctx.strokeText(text, at.x, at.y)
  ctx.fillStyle = color
  ctx.fillText(text, at.x, at.y)
  ctx.restore()
}

function arrowhead(ctx: CanvasRenderingContext2D, tip: Point, ux: number, uy: number, size = 7) {
  ctx.beginPath()
  ctx.moveTo(tip.x, tip.y)
  ctx.lineTo(tip.x - ux * size - uy * size * 0.35, tip.y - uy * size + ux * size * 0.35)
  ctx.lineTo(tip.x - ux * size + uy * size * 0.35, tip.y - uy * size - ux * size * 0.35)
  ctx.closePath()
  ctx.fill()
}

/**
 * A dimension line between `a` and `b`, offset sideways by `offset` px with
 * extension lines, arrowheads on both ends and a centered label.
 */
export function dimension(
  ctx: CanvasRenderingContext2D,
  theme: Theme,
  a: Point,
  b: Point,
  text: string,
  { offset = 0, color = theme.inkSoft }: { offset?: number; color?: string } = {},
) {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const len = Math.hypot(dx, dy)
  if (len < 1) return
  const ux = dx / len
  const uy = dy / len
  const nx = -uy * offset
  const ny = ux * offset
  const a2 = { x: a.x + nx, y: a.y + ny }
  const b2 = { x: b.x + nx, y: b.y + ny }

  ctx.save()
  ctx.strokeStyle = color
  ctx.fillStyle = color
  ctx.lineWidth = 1
  if (offset !== 0) {
    const s = Math.sign(offset) * 4
    ctx.beginPath()
    ctx.moveTo(a.x - uy * s, a.y + ux * s)
    ctx.lineTo(a2.x - uy * s, a2.y + ux * s)
    ctx.moveTo(b.x - uy * s, b.y + ux * s)
    ctx.lineTo(b2.x - uy * s, b2.y + ux * s)
    ctx.globalAlpha = 0.6
    ctx.stroke()
    ctx.globalAlpha = 1
  }
  ctx.beginPath()
  ctx.moveTo(a2.x, a2.y)
  ctx.lineTo(b2.x, b2.y)
  ctx.stroke()
  if (len > 20) {
    arrowhead(ctx, a2, -ux, -uy)
    arrowhead(ctx, b2, ux, uy)
  }
  ctx.restore()
  label(ctx, theme, text, { x: (a2.x + b2.x) / 2, y: (a2.y + b2.y) / 2 }, color)
}

/** A draggable handle: open ring with a center dot. */
export function handle(ctx: CanvasRenderingContext2D, p: Point, color: string, paper: string, r = 7) {
  ctx.save()
  ctx.beginPath()
  ctx.arc(p.x, p.y, r, 0, Math.PI * 2)
  ctx.fillStyle = paper
  ctx.fill()
  ctx.lineWidth = 2
  ctx.strokeStyle = color
  ctx.stroke()
  ctx.beginPath()
  ctx.arc(p.x, p.y, 2, 0, Math.PI * 2)
  ctx.fillStyle = color
  ctx.fill()
  ctx.restore()
}

/** Small cross marking a construction point. */
export function cross(ctx: CanvasRenderingContext2D, p: Point, color: string, size = 5) {
  ctx.save()
  ctx.strokeStyle = color
  ctx.lineWidth = 1.5
  ctx.beginPath()
  ctx.moveTo(p.x - size, p.y - size)
  ctx.lineTo(p.x + size, p.y + size)
  ctx.moveTo(p.x - size, p.y + size)
  ctx.lineTo(p.x + size, p.y - size)
  ctx.stroke()
  ctx.restore()
}
