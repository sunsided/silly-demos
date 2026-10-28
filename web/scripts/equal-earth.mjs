// The Equal Earth projection (Šavrič, Patterson & Jenny 2018) and the fit of
// Natural Earth's countries to the page's world map, shared by the scripts
// that draw onto that map, so everything lands in the same place.

const [A1, A2, A3, A4] = [1.340264, -0.081106, 0.000893, 0.003796]
const M = Math.sqrt(3) / 2

/** Longitude and latitude in degrees to projected x, y (y pointing up). */
export function project([lon, lat]) {
  const l = (lon * Math.PI) / 180
  const p = (lat * Math.PI) / 180
  const t = Math.asin(M * Math.sin(p))
  const t2 = t * t
  const t6 = t2 * t2 * t2
  const x = (l * Math.cos(t)) / (M * (A1 + 3 * A2 * t2 + t6 * (7 * A3 + 9 * A4 * t2)))
  const y = t * (A1 + A2 * t2 + t6 * (A3 + A4 * t2))
  return [x, y]
}

/** Natural Earth's admin-0 countries at `scale` ("50m", "110m"), as GeoJSON. */
export const admin0Source = scale =>
  `https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_${scale}_admin_0_countries.geojson`

/**
 * Fits projected rings to `width` px with y pointing down: returns the
 * map's height and `toPx`, which takes a projected point to the map.
 */
export function fit(rings, width) {
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity]
  for (const ring of rings)
    for (const [x, y] of ring) {
      minX = Math.min(minX, x)
      maxX = Math.max(maxX, x)
      minY = Math.min(minY, y)
      maxY = Math.max(maxY, y)
    }
  const k = width / (maxX - minX)
  return {
    height: Math.ceil((maxY - minY) * k),
    toPx: ([x, y]) => [(x - minX) * k, (maxY - y) * k],
  }
}
