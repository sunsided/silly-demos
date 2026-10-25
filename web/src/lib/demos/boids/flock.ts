import type { BoidsSim } from 'silly_demos'
import { label } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'
import type { Wasm } from '../../wasm/wasm.ts'

export interface FlockConfig {
  count: number
  separationRadius: number
  alignmentRadius: number
  cohesionRadius: number
  separationWeight: number
  alignmentWeight: number
  cohesionWeight: number
  minSpeed: number
  maxSpeed: number
  maxForce: number
  edgeMargin: number
  edgeWeight: number
  wander: number
  viewAngle: number
}

export const DEFAULT_FLOCK: FlockConfig = {
  count: 400,
  separationRadius: 28,
  alignmentRadius: 75,
  cohesionRadius: 75,
  separationWeight: 2.5,
  alignmentWeight: 1.4,
  cohesionWeight: 0.5,
  minSpeed: 60,
  maxSpeed: 140,
  maxForce: 240,
  edgeMargin: 70,
  edgeWeight: 2.5,
  wander: 40,
  viewAngle: 270,
}

export interface FlockStats {
  fps: number
  polarization: number
  avgNeighbors: number
  stragglers: number
}

/** Values per boid in `BoidsSim::state`: x, y, vx, vy, neighbors. */
export const STRIDE = 5
/** Neighbor counts at or above this are drawn as part of a dense flock. */
const DENSE = 10

export function applyConfig(wasm: Wasm, sim: BoidsSim, c: FlockConfig) {
  const p = new wasm.BoidsParams()
  p.separation_radius = c.separationRadius
  p.alignment_radius = c.alignmentRadius
  p.cohesion_radius = c.cohesionRadius
  p.separation_weight = c.separationWeight
  p.alignment_weight = c.alignmentWeight
  p.cohesion_weight = c.cohesionWeight
  p.min_speed = c.minSpeed
  p.max_speed = c.maxSpeed
  p.max_force = c.maxForce
  p.edge_margin = c.edgeMargin
  p.edge_weight = c.edgeWeight
  p.wander = c.wander
  p.view_angle = c.viewAngle
  sim.set_params(p)
  p.free()
  sim.set_count(c.count)
}

export function drawBoids(ctx: CanvasRenderingContext2D, theme: Theme, s: Float32Array, highlight: number) {
  // Motion streaks, one path for all boids.
  ctx.beginPath()
  for (let i = 0; i < s.length; i += STRIDE) {
    ctx.moveTo(s[i], s[i + 1])
    ctx.lineTo(s[i] - s[i + 2] * 0.12, s[i + 1] - s[i + 3] * 0.12)
  }
  ctx.lineWidth = 1
  ctx.strokeStyle = theme.inkFaint
  ctx.stroke()

  // Chevrons, batched by color: stragglers, loose and dense flock members.
  const buckets: [string, (n: number) => boolean][] = [
    [theme.signal, n => n === 0],
    [theme.ink, n => n > 0 && n < DENSE],
    [theme.cobalt, n => n >= DENSE],
  ]
  for (const [color, test] of buckets) {
    ctx.beginPath()
    for (let i = 0; i < s.length; i += STRIDE) {
      if (!test(s[i + 4])) continue
      const x = s[i], y = s[i + 1]
      const speed = Math.hypot(s[i + 2], s[i + 3]) || 1
      const ux = s[i + 2] / speed, uy = s[i + 3] / speed
      ctx.moveTo(x + ux * 6, y + uy * 6)
      ctx.lineTo(x - ux * 4 - uy * 3.5, y - uy * 4 + ux * 3.5)
      ctx.lineTo(x - ux * 2, y - uy * 2)
      ctx.lineTo(x - ux * 4 + uy * 3.5, y - uy * 4 - ux * 3.5)
      ctx.closePath()
    }
    ctx.fillStyle = color
    ctx.fill()
  }

  if (highlight >= 0) {
    const i = highlight * STRIDE
    ctx.beginPath()
    ctx.arc(s[i], s[i + 1], 11, 0, Math.PI * 2)
    ctx.lineWidth = 1.5
    ctx.strokeStyle = theme.signal
    ctx.stroke()
  }
}

/** Draws what the focused boid perceives: its radii, field of view and neighbors. */
export function drawPerception(ctx: CanvasRenderingContext2D, theme: Theme, s: Float32Array, focus: number, c: FlockConfig) {
  const i = focus * STRIDE
  const x = s[i], y = s[i + 1]
  const heading = Math.atan2(s[i + 3], s[i + 2])
  const halfFov = (c.viewAngle / 2) * Math.PI / 180
  const cosHalf = Math.cos(halfFov)

  ctx.save()
  // Field of view wedge out to the largest radius.
  const reach = Math.max(c.separationRadius, c.alignmentRadius, c.cohesionRadius)
  ctx.beginPath()
  ctx.moveTo(x, y)
  ctx.arc(x, y, reach, heading - halfFov, heading + halfFov)
  ctx.closePath()
  ctx.fillStyle = theme.cobaltFaint
  ctx.fill()

  const rings: [number, string, string][] = [
    [c.separationRadius, theme.signal, 'separation'],
    [c.alignmentRadius, theme.verdigris, 'alignment'],
    [c.cohesionRadius, theme.cobalt, 'cohesion'],
  ]
  ctx.setLineDash([4, 4])
  ctx.lineWidth = 1
  for (const [r, color] of rings) {
    ctx.beginPath()
    ctx.arc(x, y, r, 0, Math.PI * 2)
    ctx.strokeStyle = color
    ctx.stroke()
  }
  ctx.setLineDash([])

  // Lines to visible neighbors within the cohesion radius.
  const ux = Math.cos(heading), uy = Math.sin(heading)
  ctx.beginPath()
  for (let j = 0; j < s.length; j += STRIDE) {
    if (j === i) continue
    const dx = s[j] - x, dy = s[j + 1] - y
    const d = Math.hypot(dx, dy)
    if (d === 0 || d >= c.cohesionRadius || ux * dx + uy * dy < cosHalf * d) continue
    ctx.moveTo(x, y)
    ctx.lineTo(s[j], s[j + 1])
  }
  ctx.strokeStyle = theme.cobalt
  ctx.globalAlpha = 0.5
  ctx.stroke()
  ctx.restore()

  for (const [r, color, name] of rings) {
    label(ctx, theme, name, { x: x + r * Math.SQRT1_2 + 4, y: y - r * Math.SQRT1_2 - 4 }, color, 'left')
  }
}
