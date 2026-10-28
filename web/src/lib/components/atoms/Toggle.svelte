<!-- @component Labeled checkbox. -->
<script lang="ts">
  let { label, checked = $bindable(false) }: { label: string; checked?: boolean } = $props()
</script>

<label class="toggle">
  <input type="checkbox" bind:checked />
  <span>{label}</span>
</label>

<style>
  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    font-size: var(--font-size-md);
    cursor: pointer;
  }

  input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--color-ink);
  }
</style>
