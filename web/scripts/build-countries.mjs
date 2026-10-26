// Fetches development indicators for every country from the World Bank API
// and writes web/static/data/countries.json for the Kohonen lab's world
// poverty map: one row of values per country around 1992 and around 2022.
//
// Run with `task data:countries` (needs network access). The output is
// committed; the site build never runs this.

import { writeFile } from 'node:fs/promises'

const API = 'https://api.worldbank.org/v2'
const OUT = new URL('../static/data/countries.json', import.meta.url)

/** The indicators, in column order. `log`: trained on log10 of the value. */
const INDICATORS = [
  { code: 'NY.GDP.PCAP.PP.KD', name: 'GDP per capita (PPP)', unit: 'int. $', log: true },
  { code: 'SP.DYN.LE00.IN', name: 'Life expectancy', unit: 'years', log: false },
  { code: 'SH.DYN.MORT', name: 'Under-5 mortality', unit: 'per 1000', log: true },
  { code: 'SP.DYN.TFRT.IN', name: 'Fertility', unit: 'births per woman', log: false },
  { code: 'SP.ADO.TFRT', name: 'Adolescent fertility', unit: 'per 1000 women 15–19', log: false },
  { code: 'SP.URB.TOTL.IN.ZS', name: 'Urban population', unit: '%', log: false },
  { code: 'SP.POP.GROW', name: 'Population growth', unit: '% per year', log: false },
  { code: 'SP.POP.DPND', name: 'Age dependency', unit: '% of working age', log: false },
  { code: 'EG.ELC.ACCS.ZS', name: 'Access to electricity', unit: '%', log: false },
  { code: 'NV.AGR.TOTL.ZS', name: 'Agriculture', unit: '% of GDP', log: false },
]

/** Target year and the window of years whose values may stand in for it. */
const YEARS = [
  { key: 'y1992', target: 1992, from: 1990, to: 1995 },
  { key: 'y2022', target: 2022, from: 2019, to: 2023 },
]

/** A country is dropped when more than this share of a year's values is missing. */
const MAX_MISSING = 0.3
const MIN_COUNTRIES = 120

async function get(path) {
  const res = await fetch(`${API}/${path}`)
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`)
  const body = await res.json()
  if (!Array.isArray(body) || !Array.isArray(body[1])) throw new Error(`${path}: unexpected response`)
  return body[1]
}

/** Four significant digits, enough for a map and much smaller. */
const round = v => (v === null ? null : Number(v.toPrecision(4)))

const countries = (await get('country?format=json&per_page=400'))
  .filter(c => c.region.id !== 'NA')
  .map(c => ({ iso3: c.id, name: c.name, region: c.region.value.trim() }))

// values[code][iso3][year] = value
const values = {}
for (const { code } of INDICATORS) {
  const rows = await get(`country/all/indicator/${code}?date=1990:2023&format=json&per_page=20000`)
  values[code] = {}
  for (const r of rows) {
    if (r.value === null) continue
    ;(values[code][r.countryiso3code] ??= {})[Number(r.date)] = r.value
  }
  console.log(`${code}: ${rows.length} rows`)
}

/** The value nearest the target year within its window; ties go to the later year. */
function pick(code, iso3, { target, from, to }) {
  const byYear = values[code][iso3]
  if (!byYear) return null
  let best = null
  for (let y = from; y <= to; y++) {
    if (byYear[y] === undefined) continue
    const better =
      best === null || Math.abs(y - target) < Math.abs(best - target) ||
      (Math.abs(y - target) === Math.abs(best - target) && y > best)
    if (better) best = y
  }
  return best === null ? null : byYear[best]
}

const kept = []
const dropped = []
for (const c of countries) {
  const row = { ...c }
  let ok = true
  for (const year of YEARS) {
    row[year.key] = INDICATORS.map(({ code }) => round(pick(code, c.iso3, year)))
    const missing = row[year.key].filter(v => v === null).length
    if (missing > MAX_MISSING * INDICATORS.length) ok = false
  }
  ;(ok ? kept : dropped).push(row)
}

// How often each indicator is missing among the kept countries, to judge the indicator choice.
for (const [i, { code }] of INDICATORS.entries()) {
  const gaps = YEARS.map(y => kept.filter(c => c[y.key][i] === null).length)
  console.log(`${code}: missing ${gaps.join(' / ')} (1992 / 2022)`)
}
console.log(`kept ${kept.length}, dropped ${dropped.length}: ${dropped.map(c => c.iso3).join(' ')}`)

if (kept.length < MIN_COUNTRIES) {
  console.error(`only ${kept.length} countries; expected at least ${MIN_COUNTRIES}`)
  process.exit(1)
}

const out = {
  source: 'World Bank, World Development Indicators (https://data.worldbank.org)',
  license: 'CC BY 4.0',
  retrieved: new Date().toISOString().slice(0, 10),
  years: YEARS.map(({ key, target, from, to }) => ({ key, target, from, to })),
  indicators: INDICATORS,
  countries: kept,
}
await writeFile(OUT, JSON.stringify(out) + '\n')
console.log(`wrote ${OUT.pathname}`)
