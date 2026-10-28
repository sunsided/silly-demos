import type { Point } from '../../canvas/canvas.svelte.ts'

export type CellKind = 'grid' | 'hex'

/** Where a map's nodes sit when it is drawn as a picture of cells. */
export interface CellLayout {
  kind: CellKind
  centers: Point[]
  /** Half a grid cell's side, or a hex cell's circumradius. */
  radius: number
}

export interface Box {
  x: number
  y: number
  width: number
  height: number
}

const SQRT3 = Math.sqrt(3)

/**
 * Cells for a `w`×`h` lattice, centered in `box`. Node i sits at column
 * i % w, row ⌊i / w⌋; hex cells are pointy-top with odd rows shifted half a
 * cell right, matching the som crate's lattice coordinates.
 */
export function cellLayout(kind: CellKind, w: number, h: number, box: Box): CellLayout {
  const centers: Point[] = []
  if (kind === 'grid') {
    const side = Math.min(box.width / w, box.height / h)
    const x0 = box.x + (box.width - side * w) / 2
    const y0 = box.y + (box.height - side * h) / 2
    for (let i = 0; i < w * h; i++) {
      centers.push({ x: x0 + ((i % w) + 0.5) * side, y: y0 + (Math.floor(i / w) + 0.5) * side })
    }
    return { kind, centers, radius: side / 2 }
  }
  // Pointy-top hexagons of circumradius r: √3·r apart in a row, 1.5·r between rows.
  const r = Math.min(box.width / (SQRT3 * (w + 0.5)), box.height / (1.5 * (h - 1) + 2))
  const width = SQRT3 * r * (w + 0.5)
  const height = 1.5 * r * (h - 1) + 2 * r
  const x0 = box.x + (box.width - width) / 2 + (SQRT3 * r) / 2
  const y0 = box.y + (box.height - height) / 2 + r
  for (let i = 0; i < w * h; i++) {
    const row = Math.floor(i / w)
    const shift = row % 2 === 1 ? (SQRT3 * r) / 2 : 0
    centers.push({ x: x0 + (i % w) * SQRT3 * r + shift, y: y0 + row * 1.5 * r })
  }
  return { kind, centers, radius: r }
}

/** The outline of cell `i`. */
export function cellPath(ctx: CanvasRenderingContext2D, layout: CellLayout, i: number) {
  const { x, y } = layout.centers[i]
  const r = layout.radius
  if (layout.kind === 'grid') {
    ctx.rect(x - r, y - r, 2 * r, 2 * r)
    return
  }
  for (let k = 0; k < 6; k++) {
    const a = ((60 * k - 90) * Math.PI) / 180
    const px = x + r * Math.cos(a)
    const py = y + r * Math.sin(a)
    if (k === 0) ctx.moveTo(px, py)
    else ctx.lineTo(px, py)
  }
  ctx.closePath()
}

/** Fills every cell with `fill(i)`, and outlines them in `stroke` if given. */
export function drawCells(
  ctx: CanvasRenderingContext2D,
  layout: CellLayout,
  fill: (i: number) => string,
  stroke?: string,
) {
  ctx.save()
  for (let i = 0; i < layout.centers.length; i++) {
    ctx.beginPath()
    cellPath(ctx, layout, i)
    ctx.fillStyle = fill(i)
    ctx.fill()
    if (stroke) {
      ctx.strokeStyle = stroke
      ctx.lineWidth = 1
      ctx.stroke()
    }
  }
  ctx.restore()
}

/** The cell under `p`, or −1. */
export function cellAt(layout: CellLayout, p: Point): number {
  let best = -1
  let bestD = Infinity
  for (let i = 0; i < layout.centers.length; i++) {
    const d = Math.hypot(layout.centers[i].x - p.x, layout.centers[i].y - p.y)
    if (d < bestD) {
      bestD = d
      best = i
    }
  }
  return bestD <= layout.radius * 1.05 ? best : -1
}
