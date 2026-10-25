import type { ReactNode } from 'react'

export interface Readout {
  label: string
  value: ReactNode
  tone?: 'signal' | 'cobalt' | 'verdigris'
}

/**
 * Demo page layout: the canvas is the drawing sheet, the side panel is its
 * title block with the demo's name, live readouts and controls.
 */
export function Sheet({
  title,
  summary,
  readouts,
  canvas,
  children,
  notes,
}: {
  title: string
  summary: ReactNode
  readouts: Readout[]
  canvas: ReactNode
  children?: ReactNode
  notes?: ReactNode
}) {
  return (
    <div className="sheet">
      <div className="sheet-drawing grid-paper">{canvas}</div>
      <aside className="title-block" aria-label={`${title} details`}>
        <header className="tb-head">
          <h1 className="tb-title">{title}</h1>
          <p className="tb-summary">{summary}</p>
        </header>
        <dl className="tb-readouts">
          {readouts.map(r => (
            <div key={r.label} className="tb-readout" data-tone={r.tone}>
              <dt>{r.label}</dt>
              <dd>{r.value}</dd>
            </div>
          ))}
        </dl>
        {children && <div className="tb-controls">{children}</div>}
        {notes && <div className="tb-notes">{notes}</div>}
      </aside>
    </div>
  )
}

export function Loading({ error }: { error: string | null }) {
  return (
    <div className="sheet-status" role="status">
      {error
        ? <>The WebAssembly module failed to load: {error}. Reload the page to try again.</>
        : <>Loading the WebAssembly module…</>}
    </div>
  )
}

export function ControlGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="control-group">
      <legend>{title}</legend>
      {children}
    </fieldset>
  )
}

export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  unit,
  format = v => String(v),
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  step?: number
  unit?: string
  format?: (v: number) => string
  onChange: (v: number) => void
}) {
  return (
    <label className="slider">
      <span className="slider-label">{label}</span>
      <output className="slider-value">
        {format(value)}{unit && <span className="unit"> {unit}</span>}
      </output>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={e => onChange(parseFloat(e.target.value))}
        style={{ '--fill': `${((value - min) / (max - min)) * 100}%` } as React.CSSProperties}
      />
    </label>
  )
}

export function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  )
}

export function Select<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: T
  options: readonly { value: T; label: string }[]
  onChange: (v: T) => void
}) {
  return (
    <label className="select">
      <span className="select-label">{label}</span>
      <select value={value} onChange={e => onChange(e.target.value as T)}>
        {options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </label>
  )
}

/** A radio list whose options carry a short description. */
export function Choice<T extends string>({
  name,
  value,
  options,
  onChange,
}: {
  name: string
  value: T
  options: readonly { value: T; label: string; detail?: ReactNode }[]
  onChange: (v: T) => void
}) {
  return (
    <div className="choice" role="radiogroup">
      {options.map(o => (
        <label key={o.value} className="choice-option" data-checked={o.value === value || undefined}>
          <input type="radio" name={name} value={o.value} checked={o.value === value} onChange={() => onChange(o.value)} />
          <span className="choice-label">{o.label}</span>
          {o.detail && <span className="choice-detail">{o.detail}</span>}
        </label>
      ))}
    </div>
  )
}
