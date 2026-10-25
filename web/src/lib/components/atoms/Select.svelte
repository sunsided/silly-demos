<!-- @component Labelled native select. -->
<script lang="ts" generics="T extends string">
  let {
    label,
    value = $bindable(),
    options,
  }: { label: string; value: T; options: readonly { value: T; label: string }[] } = $props()
</script>

<label class="select">
  <span class="label">{label}</span>
  <select bind:value>
    {#each options as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
</label>

<style>
  .select {
    display: grid;
    gap: var(--space-2xs);
    font-size: var(--font-size-sm);
  }

  .label {
    color: var(--color-ink-soft);
  }

  select {
    padding: var(--space-xs) var(--space-sm);
    border: var(--border-hairline) solid var(--color-ink-faint);
    border-radius: var(--border-radius);
    background: var(--color-paper);
    color: var(--color-ink);
    font: inherit;
    font-size: var(--font-size-md);
  }
</style>
