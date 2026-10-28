/** Accent colors a component can be tinted with; matches the semantic color tokens. */
export type Tone = 'ink' | 'cobalt' | 'signal' | 'verdigris' | 'amber'

/** A labeled measurement shown in a sheet's title block. */
export interface ReadoutItem {
  label: string
  value: string | number
  tone?: Tone
}
