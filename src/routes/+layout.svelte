<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";

  let { children } = $props();

  const links = [
    { href: "/", label: "Predictions" },
    { href: "/standings", label: "Standings" },
    { href: "/stats", label: "Stats" },
    { href: "/learn", label: "Learn" },
    { href: "/playground", label: "Playground" },
  ];

  function isActive(href: string): boolean {
    if (href === "/") return page.url.pathname === "/";
    return page.url.pathname.startsWith(href);
  }
</script>

<header class="top">
  <div class="shell row">
    <a class="brand" href="/" aria-label="MLB Pythagorean Expectation home">
      <span class="diamond" aria-hidden="true"></span>
      <span class="brand-text">
        <span class="brand-title">MLB Pythagorean</span>
        <span class="brand-sub">Expectation Model</span>
      </span>
    </a>
    <nav>
      {#each links as link}
        <a class:active={isActive(link.href)} href={link.href}>{link.label}</a>
      {/each}
    </nav>
  </div>
</header>

<main class="shell main">
  {@render children?.()}
</main>

<footer class="shell footer">
  <p class="muted">
    Data from <a href="https://statsapi.mlb.com" target="_blank" rel="noopener">statsapi.mlb.com</a> · Built with Rust + Tauri ·
    Model after <em>Bill James</em> (1980)
  </p>
</footer>

<style>
  .top {
    position: sticky;
    top: 0;
    z-index: 10;
    background: color-mix(in srgb, var(--bg) 92%, transparent);
    backdrop-filter: saturate(140%) blur(10px);
    -webkit-backdrop-filter: saturate(140%) blur(10px);
    border-bottom: 1px solid var(--line-soft);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 14px;
    padding-bottom: 14px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--ink);
    text-decoration: none;
  }
  .brand:hover {
    text-decoration: none;
  }
  .diamond {
    width: 16px;
    height: 16px;
    background: var(--accent);
    transform: rotate(45deg);
    border-radius: 3px;
    box-shadow: inset 0 -2px 0 rgba(0, 0, 0, 0.18);
  }
  .brand-text {
    display: flex;
    flex-direction: column;
    line-height: 1.05;
  }
  .brand-title {
    font-family: var(--serif);
    font-weight: 600;
    font-size: 1.05rem;
  }
  .brand-sub {
    font-size: 0.72rem;
    color: var(--ink-mute);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav a {
    color: var(--ink-soft);
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 0.95rem;
  }
  nav a:hover {
    text-decoration: none;
    background: var(--bg-soft);
    color: var(--ink);
  }
  nav a.active {
    background: var(--ink);
    color: var(--bg-elev);
  }
  .main {
    padding-top: 28px;
  }
  .footer {
    padding-top: 24px;
    padding-bottom: 32px;
    margin-top: 40px;
    border-top: 1px solid var(--line-soft);
    font-size: 0.85rem;
  }
  .footer p {
    margin: 0;
  }
</style>
