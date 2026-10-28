// Downloads Natural Earth's admin-0 country shapes (public domain), projects
// them with the Equal Earth projection and writes web/static/data/world.json:
// one SVG path per country, keyed by ISO 3166-1 alpha-3 code, for the
// Kohonen lab's world poverty map.
//
// Run with `task data:world` (needs network access) after `task
// data:countries`, whose codes it checks coverage against. The output is
// committed; the site build never runs this.

import { readFile, writeFile } from 'node:fs/promises'
import { admin0Source, fit, project } from './equal-earth.mjs'

// 1:50m: 1:110m has no shapes for many small island states.
const SCALE = process.env.NE_SCALE ?? '50m'
const SOURCE = admin0Source(SCALE)
const OUT = new URL('../static/data/world.json', import.meta.url)
const COUNTRIES = new URL('../static/data/countries.json', import.meta.url)
const WIDTH = 1000
/** Points nearer than this (px) to the last one kept are dropped; finer detail is invisible at WIDTH. */
const TOLERANCE = 0.7
/** Share of the data's countries that must have a shape. */
const MIN_COVERAGE = Number(process.env.MIN_COVERAGE ?? 0.9)

/** Natural Earth codes that differ from the World Bank's. */
const RENAMES = { KOS: 'XKX', SDS: 'SSD', PSX: 'PSE' }
/** No data and a lot of paper. */
const SKIP = new Set(['ATA'])

function code(props) {
  const iso = props.ISO_A3 !== '-99' ? props.ISO_A3 : props.ADM0_A3
  return RENAMES[iso] ?? iso
}

const res = await fetch(SOURCE)
if (!res.ok) throw new Error(`${SOURCE}: HTTP ${res.status}`)
const geo = await res.json()

// Project every ring, then fit the whole map to WIDTH with y pointing down.
const shapes = []
for (const f of geo.features) {
  const iso = code(f.properties)
  if (SKIP.has(iso)) continue
  const polygons = f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates
  shapes.push({ iso, rings: polygons.flat().map(ring => ring.map(project)) })
}
const { height, toPx } = fit(
  shapes.flatMap(s => s.rings),
  WIDTH,
)
const r = v => Math.round(v * 10) / 10

/** A ring as path data, dropping points nearer than `tolerance` px to the last one kept. */
function ringPath(ring, tolerance) {
  const kept = []
  for (const [x, y] of ring) {
    const p = toPx([x, y])
    const last = kept.at(-1)
    if (!last || Math.hypot(p[0] - last[0], p[1] - last[1]) >= tolerance) kept.push(p)
  }
  return kept.length >= 3 ? `M${kept.map(([x, y]) => `${r(x)},${r(y)}`).join('L')}Z` : ''
}

const paths = {}
for (const { iso, rings } of shapes) {
  let d = rings.map(ring => ringPath(ring, TOLERANCE)).join('')
  // A small island state thins away completely; keep it at full detail as a speck.
  d ||= rings.map(ring => ringPath(ring, 0)).join('')
  if (d) paths[iso] = (paths[iso] ?? '') + d
}

const data = JSON.parse(await readFile(COUNTRIES, 'utf8'))
const codes = data.countries.map(c => c.iso3)
const missing = codes.filter(c => !paths[c])
const coverage = 1 - missing.length / codes.length
console.log(`${Object.keys(paths).length} shapes; ${(coverage * 100).toFixed(1)} % of the data's countries`)
console.log(`no shape: ${missing.join(' ')}`)
console.log(`no data: ${Object.keys(paths).filter(c => !codes.includes(c)).join(' ')}`)
if (coverage < MIN_COVERAGE) {
  console.error(`coverage below ${MIN_COVERAGE * 100} %`)
  process.exit(1)
}

const out = {
  source: `Natural Earth 1:${SCALE.replace('m', ' million')} Admin 0 countries (public domain)`,
  projection: 'Equal Earth',
  width: WIDTH,
  height,
  countries: paths,
}
await writeFile(OUT, JSON.stringify(out) + '\n')
console.log(`wrote ${OUT.pathname}`)
