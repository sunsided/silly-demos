<!-- @component Site header: wordmark, demo navigation and source link. -->
<script lang="ts">
  import { resolve } from '$app/paths'
  import { page } from '$app/state'
  import { DEMOS } from '../../demos/index.ts'
  import NavLink from '../molecules/NavLink.svelte'
</script>

<header>
  <a class="wordmark" href={resolve('/')}>silly-demos</a>
  <nav aria-label="Demos">
    {#each DEMOS as demo (demo.route)}
      <NavLink href={resolve(demo.route)} active={page.url.pathname === resolve(demo.route)}>{demo.name}</NavLink>
    {/each}
  </nav>
  <a class="source" href="https://github.com/sunsided/silly-demos">Source</a>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: var(--space-4xl);
    padding: 0 var(--space-3xl);
    border-bottom: var(--border-hairline) solid var(--color-rule);
    background: var(--color-paper-raised);
  }

  .wordmark {
    font-size: 19px;
    font-weight: var(--font-weight-heavy);
    font-stretch: var(--font-stretch-wider);
    letter-spacing: -0.02em;
    text-decoration: none;
  }

  nav {
    display: flex;
    flex: 1;
    gap: var(--space-2xs);
  }

  .source {
    padding: var(--space-xs) var(--space-md);
    color: var(--color-ink-soft);
    font-weight: var(--font-weight-medium);
    text-decoration: none;
  }

  .source:hover {
    color: var(--color-ink);
  }

  @media (max-width: 900px) {
    header {
      flex-wrap: wrap;
      gap: var(--space-2xs) var(--space-lg);
      padding: var(--space-md) var(--space-lg) 0;
    }

    nav {
      order: 3;
      flex-basis: 100%;
      margin: 0 calc(-1 * var(--space-md));
      overflow-x: auto;
      scrollbar-width: none;
    }

    .source {
      margin-left: auto;
    }
  }
</style>
