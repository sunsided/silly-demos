<!-- @component One measured value: a `dt`/`dd` pair for use inside a `dl`. -->
<script lang="ts">
  import type { Tone } from '../types.ts'

  let { label, value, tone }: { label: string; value: string | number; tone?: Tone } = $props()
</script>

<div class="readout" style:--tone={tone ? `var(--color-${tone})` : undefined}>
  <dt>{label}</dt>
  <dd>{value}</dd>
</div>

<style>
  .readout {
    padding: var(--space-md) var(--space-2xl) 12px;
    min-width: 0;
  }

  dt {
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }

  dd {
    margin: var(--space-3xs) 0 0;
    color: var(--tone, var(--color-ink));
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    font-stretch: var(--font-stretch-wide);
    line-height: 1.15;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }
</style>
