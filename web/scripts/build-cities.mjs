// Downloads Natural Earth's populated places (public domain) and writes
// web/static/data/cities.json for the Kohonen lab's Tours mode: three sets
// of real cities, each with its longitude and latitude, for great-circle
// tour lengths, and its position on the page's world map (world.json).
//
// Run with `task data:cities` (needs network access). The output is
// committed; the site build never runs this.

import { writeFile } from 'node:fs/promises'
import { admin0Source, fit, project } from './equal-earth.mjs'

const PLACES =
  'https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_populated_places_simple.geojson'
// The same scale, width and skipped country as build-world.mjs, so the fit matches world.json.
const SCALE = '50m'
const WIDTH = 1000
const SKIP = new Set(['ATA'])
const OUT = new URL('../static/data/cities.json', import.meta.url)

/** Each set, how it is chosen, and how many cities it must come to. */
const SETS = [
  { id: 'capitals', label: 'World capitals', count: [180, 210] },
  { id: 'europe', label: 'European capitals', count: [40, 50] },
  { id: 'largest', label: '100 largest cities', count: [100, 100] },
]

async function json(url) {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`)
  return res.json()
}

const [places, countries] = await Promise.all([json(PLACES), json(admin0Source(SCALE))])

// Fit exactly as build-world.mjs does: every ring of every country but Antarctica.
const rings = []
const continent = {}
for (const f of countries.features) {
  const p = f.properties
  continent[p.ADM0_A3] = p.CONTINENT
  const iso = p.ISO_A3 !== '-99' ? p.ISO_A3 : p.ADM0_A3
  if (SKIP.has(iso) || SKIP.has(p.ADM0_A3)) continue
  const polygons = f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates
  for (const ring of polygons.flat()) rings.push(ring.map(project))
}
const { toPx } = fit(rings, WIDTH)

const r = (v, digits) => Number(v.toFixed(digits))
function city(p) {
  const [x, y] = toPx(project([p.longitude, p.latitude]))
  return { name: p.name, iso3: p.adm0_a3, lon: r(p.longitude, 4), lat: r(p.latitude, 4), x: r(x, 1), y: r(y, 1) }
}

const capitals = places.features.map(f => f.properties).filter(p => p.featurecla === 'Admin-0 capital')
const chosen = {
  capitals,
  europe: capitals.filter(p => continent[p.adm0_a3] === 'Europe'),
  largest: places.features
    .map(f => f.properties)
    .sort((a, b) => b.pop_max - a.pop_max)
    .slice(0, 100),
}

const sets = []
for (const { id, label, count } of SETS) {
  const cities = chosen[id].map(city)
  console.log(`${id}: ${cities.length} cities`)
  if (cities.length < count[0] || cities.length > count[1]) {
    console.error(`${id}: expected ${count[0]}–${count[1]} cities`)
    process.exit(1)
  }
  sets.push({ id, label, cities })
}

const out = {
  source: 'Natural Earth 1:10 million populated places (public domain)',
  license: 'Public domain',
  sets,
}
await writeFile(OUT, JSON.stringify(out) + '\n')
console.log(`wrote ${OUT.pathname}`)
