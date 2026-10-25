<!-- @component One row of the demo index: name, blurb and the Rust entry point. -->
<script lang="ts">
  let { href, name, blurb, rust }: { href: string; name: string; blurb: string; rust: string } = $props()
</script>

<a {href}>
  <span class="name">{name}</span>
  <span class="blurb">{blurb}</span>
  <code>{rust}</code>
</a>

<style>
  a {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(0, 2fr) minmax(0, 1fr);
    gap: var(--space-3xl);
    align-items: baseline;
    padding: 22px var(--space-3xl);
    text-decoration: none;
  }

  a:hover {
    background: var(--color-cobalt-faint);
  }

  a:hover .name {
    color: var(--color-cobalt);
  }

  .name {
    font-size: var(--font-size-2xl);
    font-weight: 750;
    font-stretch: 118%;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .blurb {
    max-width: 62ch;
    color: var(--color-ink-soft);
  }

  code {
    justify-self: end;
    color: var(--color-ink-soft);
  }

  @media (max-width: 900px) {
    a {
      grid-template-columns: 1fr;
      gap: var(--space-xs);
      padding: var(--space-xl) var(--space-lg);
    }

    code {
      justify-self: start;
    }
  }
</style>
