import { asset } from '$app/paths'

export interface Indicator {
  code: string
  name: string
  unit: string
  /** Trained on log10 of the value. */
  log: boolean
}

export interface Country {
  iso3: string
  name: string
  region: string
  y1992: (number | null)[]
  y2022: (number | null)[]
}

export interface CountryData {
  source: string
  license: string
  retrieved: string
  indicators: Indicator[]
  countries: Country[]
}

export interface World {
  source: string
  width: number
  height: number
  /** SVG path data per ISO 3166-1 alpha-3 code. */
  countries: Record<string, string>
}

export type YearView = '1992' | '2022' | 'both'

/** The component planes of a trained map, one per indicator, in data units. */
export interface Planes {
  kind: 'grid' | 'hex'
  width: number
  height: number
  indicators: Indicator[]
  values: Float32Array[]
}

async function json<T>(path: Parameters<typeof asset>[0]): Promise<T> {
  const res = await fetch(asset(path))
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`)
  return res.json() as Promise<T>
}

/** The bundled World Bank indicators (static/data/countries.json). */
export const loadCountries = () => json<CountryData>('data/countries.json')

/** The bundled Natural Earth shapes (static/data/world.json). */
export const loadWorld = () => json<World>('data/world.json')

/**
 * Rows to train on: every country in 1992, then every country in 2022, with
 * log10 of the log-scaled indicators and NaN for missing values. Row i is
 * country i % n in year ⌊i / n⌋.
 */
export function trainingRows(data: CountryData): { rows: Float32Array; dim: number } {
  const dim = data.indicators.length
  const n = data.countries.length
  const rows = new Float32Array(2 * n * dim)
  for (const [y, key] of (['y1992', 'y2022'] as const).entries()) {
    data.countries.forEach((c, i) => {
      c[key].forEach((v, k) => {
        rows[(y * n + i) * dim + k] = v === null ? NaN : data.indicators[k].log ? Math.log10(v) : v
      })
    })
  }
  return { rows, dim }
}

/** Codes with data and a shape, with data only, and with a shape only. */
export function joinShapes(data: CountryData, world: World) {
  const codes = new Set(data.countries.map(c => c.iso3))
  return {
    withShape: [...codes].filter(c => world.countries[c]),
    withoutShape: [...codes].filter(c => !world.countries[c]),
    shapeOnly: Object.keys(world.countries).filter(c => !codes.has(c)),
  }
}

/**
 * A colour for a position on the map, as in Kaski and Kohonen's figure: the
 * direction from the map's centre sets the hue, the distance the saturation.
 * `dx`, `dy` are relative to the centre, `maxR` the farthest any cell is.
 */
export function wheelColour(dx: number, dy: number, maxR: number, dark: boolean): string {
  const hue = ((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360
  const sat = 20 + 70 * Math.min(1, Math.hypot(dx, dy) / maxR)
  return `hsl(${hue.toFixed(0)} ${sat.toFixed(0)}% ${dark ? 62 : 55}%)`
}

/** A value for the readout table; missing values show as a dash. */
export function formatValue(v: number | null | undefined, unit: string): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '–'
  const abs = Math.abs(v)
  const digits = abs >= 1000 ? 0 : abs >= 100 ? 0 : abs >= 10 ? 1 : 2
  const text = v.toLocaleString('en', { maximumFractionDigits: digits, minimumFractionDigits: 0 })
  return unit === '%' ? `${text} %` : text
}
