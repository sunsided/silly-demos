import { useEffect, useState } from 'react'
import init, * as wasm from 'silly_demos'

export type Wasm = typeof wasm

let loading: Promise<Wasm> | null = null

/** Initializes the WASM module once and shares it across demos. */
export function loadWasm(): Promise<Wasm> {
  loading ??= init().then(() => wasm)
  return loading
}

export function useWasm(): { wasm: Wasm | null; error: string | null } {
  const [wasm, setWasm] = useState<Wasm | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    loadWasm()
      .then(m => { if (!cancelled) setWasm(m) })
      .catch((e: unknown) => {
        console.error('Failed to load WASM module:', e)
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      })
    return () => { cancelled = true }
  }, [])

  return { wasm, error }
}
