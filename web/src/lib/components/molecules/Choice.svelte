<!-- @component Radio list whose options can carry a short description. -->
<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte'

  let {
    name,
    value = $bindable(),
    options,
  }: {
    name: string
    value: T
    options: readonly { value: T; label: string; detail?: Snippet | string }[]
  } = $props()
</script>

<div class="choice" role="radiogroup">
  {#each options as option (option.value)}
    <label class="option" data-checked={option.value === value || undefined}>
      <input type="radio" {name} value={option.value} bind:group={value} />
      <span class="label">{option.label}</span>
      {#if typeof option.detail === 'string'}
        <span class="detail">{option.detail}</span>
      {:else if option.detail}
        <span class="detail">{@render option.detail()}</span>
      {/if}
    </label>
  {/each}
</div>

<style>
  .choice {
    display: flex;
    flex-direction: column;
    gap: var(--space-3xs);
  }

  .option {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: var(--space-sm);
    padding: var(--space-xs) var(--space-sm);
    border-left: var(--border-accent) solid transparent;
    font-size: var(--font-size-md);
    cursor: pointer;
  }

  .option[data-checked] {
    border-left-color: var(--color-ink);
    background: var(--color-cobalt-faint);
  }

  input {
    margin: var(--space-3xs) 0 0;
    accent-color: var(--color-ink);
  }

  .label {
    font-weight: var(--font-weight-semibold);
  }

  .detail {
    grid-column: 2;
    margin-top: var(--space-3xs);
    color: var(--color-ink-soft);
    font-size: var(--font-size-sm);
  }
</style>
