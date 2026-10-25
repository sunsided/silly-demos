<!-- @component Range input with its label and formatted value; the thumb matches canvas handles. -->
<script lang="ts">
  let {
    label,
    value = $bindable(),
    min,
    max,
    step = 1,
    unit,
    format = (v: number) => String(v),
  }: {
    label: string
    value: number
    min: number
    max: number
    step?: number
    unit?: string
    format?: (v: number) => string
  } = $props()

  let fill = $derived(`${max > min ? ((value - min) / (max - min)) * 100 : 0}%`)
</script>

<label class="slider">
  <span class="label">{label}</span>
  <output class="value">
    {format(value)}{#if unit}<span class="unit"> {unit}</span>{/if}
  </output>
  <input type="range" {min} {max} {step} bind:value style:--fill={fill} />
</label>

<style>
  .slider {
    display: grid;
    grid-template-columns: 1fr auto;
    row-gap: var(--space-2xs);
    font-size: var(--font-size-sm);
  }

  .label {
    color: var(--color-ink-soft);
  }

  .value {
    font-weight: var(--font-weight-semibold);
  }

  .unit {
    font-weight: var(--font-weight-regular);
    color: var(--color-ink-soft);
  }

  input {
    grid-column: 1 / -1;
    width: 100%;
    height: 18px;
    margin: 0;
    background: transparent;
    appearance: none;
    cursor: pointer;
  }

  input::-webkit-slider-runnable-track {
    height: 2px;
    background: linear-gradient(to right, var(--color-ink) var(--fill), var(--color-ink-faint) var(--fill));
  }

  input::-moz-range-track {
    height: 2px;
    background: linear-gradient(to right, var(--color-ink) var(--fill), var(--color-ink-faint) var(--fill));
  }

  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -6px;
    border: var(--border-accent) solid var(--color-ink);
    border-radius: 50%;
    background: var(--color-paper-raised);
  }

  input::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border: var(--border-accent) solid var(--color-ink);
    border-radius: 50%;
    background: var(--color-paper-raised);
  }

  input:focus-visible {
    outline-offset: var(--space-2xs);
  }
</style>
