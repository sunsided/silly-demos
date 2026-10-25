<!--
  @component
  Renders `children` with the WASM module once it has loaded, and a status
  line until then (which is also what prerendering captures).
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import StatusMessage from '../components/atoms/StatusMessage.svelte'
  import { loadWasm, type Wasm } from './wasm.ts'

  let { children }: { children: Snippet<[Wasm]> } = $props()

  let wasm = $state.raw<Wasm | null>(null)
  let error = $state<string | null>(null)

  $effect(() => {
    let cancelled = false
    loadWasm().then(
      m => {
        if (!cancelled) wasm = m
      },
      (e: unknown) => {
        console.error('Failed to load WASM module:', e)
        if (!cancelled) error = e instanceof Error ? e.message : String(e)
      },
    )
    return () => {
      cancelled = true
    }
  })
</script>

{#if wasm}
  {@render children(wasm)}
{:else}
  <StatusMessage>
    {#if error}
      The WebAssembly module failed to load: {error}. Reload the page to try again.
    {:else}
      Loading the WebAssembly module…
    {/if}
  </StatusMessage>
{/if}
