import type { Tone } from '../components/types.ts'

/** Recall@10 and distances per query at each search effort, and what a scan takes. */
export interface SearchCurve {
  points: { effort: number; recall: number; cost: number }[]
  scan: number
}

/** A curve as the search chart draws it: its label, color and what its effort is called. */
export interface ChartCurve {
  label: string
  tone: Tone
  /** What the effort is, as in "beam 4" or "ef 32". */
  effort: string
  curve: SearchCurve
}

/** A curve from a lab's search(): [effort, recall, cost] each, then the scan. */
export function readSearchCurve(s: Float32Array): SearchCurve | null {
  if (s.length === 0) return null
  const points = []
  for (let i = 0; i + 3 <= s.length - 1; i += 3) points.push({ effort: s[i], recall: s[i + 1], cost: s[i + 2] })
  return { points, scan: s[s.length - 1] }
}
