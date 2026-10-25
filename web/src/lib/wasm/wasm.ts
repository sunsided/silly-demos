export type Wasm = typeof import('silly_demos')

let loading: Promise<Wasm> | null = null

/**
 * Loads and initialises the WASM module once, shared by all demos. Browser
 * only: pages are prerendered without it and load it after hydration.
 */
export function loadWasm(): Promise<Wasm> {
  loading ??= import('silly_demos').then(async wasm => {
    await wasm.default()
    return wasm
  })
  return loading
}
