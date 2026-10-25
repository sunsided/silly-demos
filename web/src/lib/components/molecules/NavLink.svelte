<!-- @component Masthead link; the current page gets a tab sitting on the masthead rule. -->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { href, active = false, children }: { href: string; active?: boolean; children: Snippet } = $props()
</script>

<a {href} class={{ active }} aria-current={active ? 'page' : undefined}>{@render children()}</a>

<style>
  a {
    position: relative;
    padding: var(--space-xs) var(--space-md);
    color: var(--color-ink-soft);
    font-weight: var(--font-weight-medium);
    text-decoration: none;
    white-space: nowrap;
  }

  a:hover,
  .active {
    color: var(--color-ink);
  }

  .active::after {
    content: '';
    position: absolute;
    left: var(--space-md);
    right: var(--space-md);
    bottom: calc((var(--size-masthead) - 100%) / -2 - var(--border-hairline));
    height: var(--border-tab);
    background: var(--color-ink);
  }

  @media (max-width: 900px) {
    .active::after {
      bottom: calc(-1 * var(--border-hairline));
    }
  }
</style>
