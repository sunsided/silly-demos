import { useEffect, useRef } from 'react'

/** Canvas colors, read from the CSS custom properties so canvas and page share one palette. */
export interface Theme {
  paper: string
  ink: string
  inkSoft: string
  inkFaint: string
  cobalt: string
  cobaltFaint: string
  signal: string
  signalFaint: string
  verdigris: string
  font: string
}

function readTheme(): Theme {
  const css = getComputedStyle(document.documentElement)
  const v = (name: string) => css.getPropertyValue(name).trim()
  return {
    paper: v('--paper'),
    ink: v('--ink'),
    inkSoft: v('--ink-soft'),
    inkFaint: v('--ink-faint'),
    cobalt: v('--cobalt'),
    cobaltFaint: v('--cobalt-faint'),
    signal: v('--signal'),
    signalFaint: v('--signal-faint'),
    verdigris: v('--verdigris'),
    font: v('--font-sans'),
  }
}

/** Returns a ref holding the current theme, refreshed when the color scheme changes. */
export function useTheme() {
  const theme = useRef<Theme>(readTheme())
  useEffect(() => {
    const query = window.matchMedia('(prefers-color-scheme: dark)')
    const update = () => { theme.current = readTheme() }
    query.addEventListener('change', update)
    return () => query.removeEventListener('change', update)
  }, [])
  return theme
}
