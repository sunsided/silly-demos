// Downloads four classic instances from TSPLIB (Gerhard Reinelt, Universität
// Heidelberg) with their proven optimal tours, and writes
// web/static/data/tsplib.json for the Kohonen lab's Tours mode.
//
// Every optimal tour is measured in TSPLIB's EUC_2D metric (distances rounded
// to the nearest integer) and must come out at the published optimum, or the
// script fails: that is what makes the page's gaps honest.
//
// Run with `task data:tsplib` (needs network access). The output is
// committed; the site build never runs this.

import { writeFile } from 'node:fs/promises'
import { gunzipSync } from 'node:zlib'

const BASE = 'https://comopt.ifi.uni-heidelberg.de/software/TSPLIB95/tsp'
const OUT = new URL('../static/data/tsplib.json', import.meta.url)

/** The instances and their published optima. */
const INSTANCES = [
  { name: 'berlin52', optimum: 7542 },
  { name: 'eil51', optimum: 426 },
  { name: 'st70', optimum: 675 },
  { name: 'kroA100', optimum: 21282 },
]

async function get(file) {
  const res = await fetch(`${BASE}/${file}.gz`)
  if (!res.ok) throw new Error(`${file}: HTTP ${res.status}`)
  return gunzipSync(Buffer.from(await res.arrayBuffer())).toString('latin1')
}

/** `KEY : value` header lines, and the lines of the named section. */
function parse(text, section) {
  const header = {}
  const body = []
  let inside = false
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim()
    if (!line || line === 'EOF') continue
    if (/^[A-Z_]+_SECTION$/.test(line)) {
      inside = line === section
      continue
    }
    const kv = !inside && line.match(/^([A-Z_]+)\s*:\s*(.*)$/)
    if (kv) header[kv[1]] = kv[2].trim()
    else if (inside) body.push(line)
  }
  return { header, body }
}

const nint = d => Math.floor(d + 0.5)

const instances = []
for (const { name, optimum } of INSTANCES) {
  const tsp = parse(await get(`${name}.tsp`), 'NODE_COORD_SECTION')
  if (tsp.header.EDGE_WEIGHT_TYPE !== 'EUC_2D') throw new Error(`${name}: not EUC_2D`)
  const n = Number(tsp.header.DIMENSION)
  const xy = tsp.body.flatMap(line => line.split(/\s+/).slice(1, 3).map(Number))
  if (xy.length !== 2 * n || xy.some(v => !Number.isFinite(v))) throw new Error(`${name}: bad coordinates`)

  const opt = parse(await get(`${name}.opt.tour`), 'TOUR_SECTION')
  const optTour = opt.body
    .flatMap(line => line.split(/\s+/).map(Number))
    .filter(v => v !== -1)
    .map(v => v - 1)
  if (optTour.length !== n || new Set(optTour).size !== n || optTour.some(c => !(c >= 0 && c < n)))
    throw new Error(`${name}: the optimal tour is not a tour of ${n} cities`)

  let length = 0
  for (let k = 0; k < n; k++) {
    const [a, b] = [optTour[k], optTour[(k + 1) % n]]
    length += nint(Math.hypot(xy[2 * a] - xy[2 * b], xy[2 * a + 1] - xy[2 * b + 1]))
  }
  if (length !== optimum) throw new Error(`${name}: the optimal tour measures ${length}, not ${optimum}`)
  console.log(`${name}: ${n} cities, optimum ${optimum} confirmed`)
  instances.push({ name, comment: tsp.header.COMMENT ?? '', optimum, xy, optTour })
}

const out = {
  source: 'TSPLIB (https://comopt.ifi.uni-heidelberg.de/software/TSPLIB95/)',
  attribution: 'TSPLIB, Gerhard Reinelt, Universität Heidelberg',
  metric: 'EUC_2D',
  instances,
}
await writeFile(OUT, JSON.stringify(out) + '\n')
console.log(`wrote ${OUT.pathname}`)
