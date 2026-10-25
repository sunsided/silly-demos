import { arrowhead, cross, dimension, label } from '../lib/draft'
import type { Theme } from '../lib/theme'

/** Palette order matches `Tone` in `src/collision/report.rs`. */
const tones = (t: Theme) => [t.ink, t.inkSoft, t.inkFaint, t.cobalt, t.signal, t.verdigris, t.amber]

/** `Tone::Faint` */
const FAINT = 2
const DASHED = 1
const FILLED = 2

/** Each problem is logged once, not on every frame. */
const warned = new Set<string>()
function warnOnce(message: string) {
  if (warned.has(message)) return
  warned.add(message)
  console.warn(`drawNotes: ${message}`)
}

/**
 * Draws the construction notes a method recorded. The buffer holds, per note,
 * `[kind, tone, flags, label, count, ...count floats]`; see `Note::encode`.
 */
export function drawNotes(ctx: CanvasRenderingContext2D, t: Theme, buf: Float32Array, labels: string[]) {
  const palette = tones(t)
  for (let i = 0; i < buf.length;) {
    // A short buffer means the encoder and this decoder disagree; stop rather than read garbage.
    if (i + 5 > buf.length) {
      warnOnce('truncated note header')
      return
    }
    const [kind, tone, flags, text, count] = buf.subarray(i, i + 5)
    if (i + 5 + count > buf.length) {
      warnOnce(`note of kind ${kind} claims ${count} values past the end of the buffer`)
      return
    }
    const d = buf.subarray(i + 5, i + 5 + count)
    i += 5 + count
    const color = palette[tone] ?? t.ink
    const caption = text >= 0 ? labels[text] : ''

    // Kinds drawn by the shared drafting helpers.
    if (kind === 1) {
      dimension(ctx, t, { x: d[0], y: d[1] }, { x: d[2], y: d[3] }, caption, { offset: d[4], color })
      continue
    }
    if (kind === 2) {
      cross(ctx, { x: d[0], y: d[1] }, color, 4)
      continue
    }
    if (kind === 5) {
      label(ctx, t, caption, { x: d[0], y: d[1] }, color)
      continue
    }

    ctx.save()
    ctx.strokeStyle = color
    ctx.fillStyle = color
    ctx.lineWidth = 1
    if (flags & DASHED) ctx.setLineDash([5, 4])
    switch (kind) {
      case 0: // segment
        ctx.beginPath()
        ctx.moveTo(d[0], d[1])
        ctx.lineTo(d[2], d[3])
        ctx.lineWidth = flags & DASHED ? 1 : 2
        ctx.stroke()
        break
      case 3: // circle
        ctx.beginPath()
        ctx.arc(d[0], d[1], d[2], 0, Math.PI * 2)
        ctx.stroke()
        break
      case 4: { // polygon
        if (count < 4) break
        ctx.beginPath()
        ctx.moveTo(d[0], d[1])
        for (let k = 2; k < count; k += 2) ctx.lineTo(d[k], d[k + 1])
        ctx.closePath()
        if (flags & FILLED) {
          ctx.globalAlpha = 0.14
          ctx.fill()
          ctx.globalAlpha = 1
        }
        // Faint polygons are reference grids (quadtree cells); keep them hairline.
        ctx.lineWidth = flags & DASHED || tone === FAINT ? 1 : 1.5
        ctx.stroke()
        break
      }
      case 6: { // interval along a line, with end ticks
        const [ox, oy, ux, uy, t0, t1] = d
        const a = { x: ox + ux * t0, y: oy + uy * t0 }
        const b = { x: ox + ux * t1, y: oy + uy * t1 }
        ctx.lineWidth = 3
        ctx.beginPath()
        ctx.moveTo(a.x, a.y)
        ctx.lineTo(b.x, b.y)
        ctx.stroke()
        ctx.lineWidth = 1.5
        ctx.beginPath()
        for (const p of [a, b]) {
          ctx.moveTo(p.x - uy * 5, p.y + ux * 5)
          ctx.lineTo(p.x + uy * 5, p.y - ux * 5)
        }
        ctx.stroke()
        break
      }
      case 7: { // arrow
        const [ax, ay, bx, by] = d
        const len = Math.hypot(bx - ax, by - ay)
        if (len < 1) break
        ctx.lineWidth = 1.5
        ctx.beginPath()
        ctx.moveTo(ax, ay)
        ctx.lineTo(bx, by)
        ctx.stroke()
        arrowhead(ctx, { x: bx, y: by }, (bx - ax) / len, (by - ay) / len)
        break
      }
      default:
        // A kind added to `Note::encode` without a case here.
        warnOnce(`unknown note kind ${kind}`)
    }
    ctx.restore()
  }
}
