import { asset } from '$app/paths'
import type { TspLab } from 'silly_demos'
import type { Wasm } from '../../wasm/wasm.ts'

export type MethodId = 'ring-som' | 'elastic' | 'growing' | 'nn' | 'two-opt' | 'exact'
export type MetricId = 'euclid' | 'tsplib' | 'geo'
export type CitySourceId = 'random' | 'shapes' | 'tsplib' | 'real'
export type TourStatus = 'empty' | 'ready' | 'running' | 'done' | 'too-big'

/** The exact solver's limit (som::tsp::exact::MAX_EXACT). */
export const MAX_EXACT = 13

export const METHODS: readonly { value: MethodId; label: string; detail: string; readout?: string }[] = [
  {
    value: 'ring-som',
    label: 'Kohonen ring',
    detail:
      'A self-organizing map on a closed ring of nodes: shown one city at a time, the nearest node and its ring neighbors move towards it, and as the neighborhood σ shrinks the ring settles onto a tour.',
    readout: 'σ',
  },
  {
    value: 'elastic',
    label: 'Elastic net',
    detail:
      'Durbin and Willshaw, 1987. Every city pulls every node, its pull shared among them by a Gaussian of width K, while the ring’s tension keeps neighbors together. As K shrinks, each city ends up with a node of its own.',
    readout: 'K',
  },
  {
    value: 'growing',
    label: 'Growing ring',
    detail:
      'Angéniol, de La Croix Vaubois and Le Texier, 1988. It starts as one node; a node that wins two cities in a pass splits, one that wins none for three passes goes, and the reach G of each pull shrinks after every pass.',
    readout: 'Nodes',
  },
  {
    value: 'nn',
    label: 'Nearest neighbor',
    detail: 'Start at a city and always go to the nearest one not yet visited. Quick, and typically 20–25 % too long.',
  },
  {
    value: 'two-opt',
    label: '2-opt + Or-opt',
    detail:
      'The nearest-neighbor tour, improved until no move helps: 2-opt swaps two edges for two shorter ones, Or-opt moves a stretch of 1–3 cities elsewhere.',
  },
  {
    value: 'exact',
    label: 'Exact (Held–Karp)',
    detail: `The shortest tour, by dynamic programming over subsets of cities: affordable up to ${MAX_EXACT} cities.`,
  },
]

export const methodLabel = (id: MethodId) => METHODS.find(m => m.value === id)?.label ?? id

/** Iterations per frame that make each method watchable; the page's speed slider scales them. */
export const BASE_SPEED: Record<MethodId, number> = {
  'ring-som': 40,
  elastic: 4,
  growing: 20,
  nn: 1,
  'two-opt': 2,
  exact: 1,
}

/** A method's parameter, as TspLab.params() describes it. */
export interface Param {
  id: string
  label: string
  value: number
  default: number
  min: number
  max: number
}

export const readParams = (lab: Pick<TspLab, 'params'>): Param[] => JSON.parse(lab.params()) as Param[]

/** The cities a tour is made of, and what is known about it. */
export interface CitySet {
  label: string
  /** x, y per city, in the drawing's coordinates. */
  xy: Float32Array
  metric: MetricId
  /** Longitude, latitude per city for `geo`; empty otherwise. */
  lonlat: Float32Array
  /** The shortest tour's length, when known. */
  optimum: number | null
  /** Where to draw: the cities' bounds, or a fixed square for editable sets. */
  bounds: Bounds
  editable: boolean
}

export interface Bounds {
  x: number
  y: number
  width: number
  height: number
}

export interface TsplibInstance {
  name: string
  comment: string
  optimum: number
  xy: number[]
  optTour: number[]
}

export interface TsplibData {
  source: string
  attribution: string
  instances: TsplibInstance[]
}

export interface RealCity {
  name: string
  iso3: string
  lon: number
  lat: number
  x: number
  y: number
}

export interface CityData {
  source: string
  license: string
  sets: { id: string; label: string; cities: RealCity[] }[]
}

async function json<T>(path: Parameters<typeof asset>[0]): Promise<T> {
  const res = await fetch(asset(path))
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`)
  return res.json() as Promise<T>
}

/** The bundled TSPLIB instances (static/data/tsplib.json). */
export const loadTsplib = () => json<TsplibData>('data/tsplib.json')

/** The bundled real-city sets (static/data/cities.json). */
export const loadCities = () => json<CityData>('data/cities.json')

/** The square random and shape sets are made in (som::tsp::shapes::SIDE). */
const SIDE = 1000
const SQUARE: Bounds = { x: 0, y: 0, width: SIDE, height: SIDE }

/** The bounds of `xy`, grown by `margin` of the larger side on every side. */
export function boundsOf(xy: ArrayLike<number>, margin = 0.06): Bounds {
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity]
  for (let i = 0; i < xy.length; i += 2) {
    minX = Math.min(minX, xy[i])
    maxX = Math.max(maxX, xy[i])
    minY = Math.min(minY, xy[i + 1])
    maxY = Math.max(maxY, xy[i + 1])
  }
  const pad = Math.max(maxX - minX, maxY - minY, 1) * margin
  return { x: minX - pad, y: minY - pad, width: maxX - minX + 2 * pad, height: maxY - minY + 2 * pad }
}

/** The shortest tour of up to MAX_EXACT cities, by asking the exact solver. */
function exactOptimum(wasm: Wasm, xy: Float32Array, metric: MetricId, lonlat: Float32Array): number | null {
  if (xy.length / 2 > MAX_EXACT || xy.length < 6) return null
  const lab = new wasm.TspLab(1)
  try {
    if (!lab.set_cities(xy, metric, lonlat) || !lab.set_method('exact')) return null
    lab.step(1)
    return lab.length()
  } finally {
    lab.free()
  }
}

/** Random cities: uniform, or in clusters. */
export function randomSet(wasm: Wasm, kind: 'uniform' | 'clusters', n: number, seed: number): CitySet {
  const xy = wasm.TspLab.shape(kind, n, seed)
  return {
    label: kind === 'uniform' ? `${n} random cities` : `${n} cities in clusters`,
    xy,
    metric: 'euclid',
    lonlat: new Float32Array(),
    optimum: exactOptimum(wasm, xy, 'euclid', new Float32Array()),
    bounds: SQUARE,
    editable: true,
  }
}

/** A shape; circles and even grids have a known shortest tour. */
export function shapeSet(wasm: Wasm, kind: 'circle' | 'grid' | 'spiral', n: number): CitySet {
  const xy = wasm.TspLab.shape(kind, n, 1)
  const known = wasm.TspLab.shape_optimum(kind, n)
  return {
    label: `${kind[0].toUpperCase()}${kind.slice(1)}, ${xy.length / 2} cities`,
    xy,
    metric: 'euclid',
    lonlat: new Float32Array(),
    optimum: Number.isNaN(known) ? exactOptimum(wasm, xy, 'euclid', new Float32Array()) : known,
    bounds: SQUARE,
    editable: true,
  }
}

/** A TSPLIB instance, measured in its own rounded metric against its published optimum. */
export function tsplibSet(instance: TsplibInstance): CitySet {
  const xy = Float32Array.from(instance.xy)
  return {
    label: instance.name,
    xy,
    metric: 'tsplib',
    lonlat: new Float32Array(),
    optimum: instance.optimum,
    bounds: boundsOf(xy),
    editable: true,
  }
}

/** Real cities on the world map, measured in great-circle km. */
export function realSet(set: CityData['sets'][number]): CitySet {
  const xy = Float32Array.from(set.cities.flatMap(c => [c.x, c.y]))
  return {
    label: set.label,
    xy,
    metric: 'geo',
    lonlat: Float32Array.from(set.cities.flatMap(c => [c.lon, c.lat])),
    optimum: null,
    bounds: boundsOf(xy, 0.08),
    editable: false,
  }
}

/** An edited set: plain distances, and only an exact solve can say what is shortest. */
export function customSet(wasm: Wasm, from: CitySet, xy: Float32Array): CitySet {
  return {
    label: `Custom, ${xy.length / 2} cities`,
    xy,
    metric: 'euclid',
    lonlat: new Float32Array(),
    optimum: exactOptimum(wasm, xy, 'euclid', new Float32Array()),
    bounds: from.bounds,
    editable: true,
  }
}

/** A tour length in the set's units: km for real cities, whole numbers for TSPLIB. */
export function formatLength(length: number, metric: MetricId): string {
  if (!Number.isFinite(length)) return '–'
  if (metric === 'geo') return `${Math.round(length).toLocaleString()} km`
  if (metric === 'tsplib') return Math.round(length).toLocaleString()
  return length.toLocaleString(undefined, { maximumFractionDigits: 0 })
}

/** How much longer than `reference` a tour is, in per cent. */
export function formatGap(length: number, reference: number | null): string {
  if (!Number.isFinite(length) || reference === null || !(reference > 0)) return '–'
  const gap = (length / reference - 1) * 100
  return `${Math.abs(gap) < 0.05 ? '0.0' : gap.toFixed(1)} %`
}

/** What the page shows about a lab. */
export interface TourStats {
  status: TourStatus
  iterations: number
  length: number
  /** σ, K or the node count; NaN for a heuristic. */
  readout: number
}

export function readTourStats(lab: TspLab): TourStats {
  return {
    status: lab.status() as TourStatus,
    iterations: lab.iterations(),
    length: lab.length(),
    readout: lab.readout(),
  }
}

/** A lab for `method` on `set`; null if the set is refused. */
export function makeLab(wasm: Wasm, set: CitySet, method: MethodId, seed: number): TspLab | null {
  const lab = new wasm.TspLab(seed)
  if (!lab.set_method(method) || !lab.set_cities(set.xy, set.metric, set.lonlat)) {
    lab.free()
    return null
  }
  return lab
}

/** One row of a race. */
export interface RaceRow {
  method: MethodId
  status: TourStatus
  length: number
  iterations: number
  /** Time spent stepping, ms. */
  time: number
}

/**
 * Every method on the same cities, stepped in turn within a time budget
 * per frame until all are done. Methods that can't run (exact on too many
 * cities) are left out.
 */
export class Race {
  rows: RaceRow[] = []
  private labs: { method: MethodId; lab: TspLab; time: number }[] = []

  constructor(wasm: Wasm, set: CitySet, seed: number) {
    for (const { value } of METHODS) {
      const lab = makeLab(wasm, set, value, seed)
      if (!lab) continue
      if (lab.status() === 'too-big') {
        lab.free()
        continue
      }
      this.labs.push({ method: value, lab, time: 0 })
    }
    this.read()
  }

  get done(): boolean {
    return this.labs.every(l => l.lab.status() === 'done')
  }

  /** Steps the unfinished labs round-robin for about `budget` ms. */
  run(budget: number) {
    const end = performance.now() + budget
    let progressed = true
    while (performance.now() < end && progressed) {
      progressed = false
      for (const l of this.labs) {
        if (l.lab.status() === 'done') continue
        const start = performance.now()
        l.lab.step(BASE_SPEED[l.method] * 4)
        l.time += performance.now() - start
        progressed = true
      }
    }
    this.read()
  }

  /** The ring and tour of a method's lab, for drawing. */
  shape(method: MethodId): { ring: Float32Array; tour: Uint32Array } | null {
    const l = this.labs.find(l => l.method === method)
    return l ? { ring: l.lab.ring(), tour: l.lab.tour() } : null
  }

  private read() {
    this.rows = this.labs.map(({ method, lab, time }) => ({
      method,
      status: lab.status() as TourStatus,
      length: lab.length(),
      iterations: lab.iterations(),
      time,
    }))
  }

  free() {
    for (const l of this.labs) l.lab.free()
    this.labs = []
  }
}
