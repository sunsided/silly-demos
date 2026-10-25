<!-- @component Text button. `primary` for the main action, `quiet` for low-emphasis ones. -->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLButtonAttributes } from 'svelte/elements'

  let {
    variant = 'default',
    children,
    ...rest
  }: HTMLButtonAttributes & { variant?: 'default' | 'primary' | 'quiet'; children: Snippet } = $props()
</script>

<button type="button" class={['button', variant]} {...rest}>{@render children()}</button>

<style>
  .button {
    padding: 7px var(--space-lg);
    border: var(--border-hairline) solid var(--color-ink);
    border-radius: var(--border-radius);
    background: transparent;
    color: var(--color-ink);
    font: inherit;
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    cursor: pointer;
  }

  .button:hover {
    background: var(--color-cobalt-faint);
  }

  .primary {
    min-width: 88px;
    background: var(--color-ink);
    color: var(--color-paper-raised);
  }

  .primary:hover {
    background: var(--color-cobalt);
    border-color: var(--color-cobalt);
  }

  .quiet {
    border-color: transparent;
    color: var(--color-ink-soft);
  }

  .quiet:hover {
    color: var(--color-ink);
    background: transparent;
    text-decoration: underline;
  }
</style>
