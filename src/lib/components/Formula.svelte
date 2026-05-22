<script lang="ts">
  let { children, label = "" } = $props();
</script>

<div class="formula" role="math" aria-label={label}>
  {@render children?.()}
</div>

<style>
  .formula {
    font-family: var(--serif);
    font-size: 1.2rem;
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    padding: 14px 18px;
    border-radius: var(--radius);
    margin: 0.6em 0 1.2em;
    overflow-x: auto;
    line-height: 1.8;
  }
  :global(.formula em) {
    font-style: italic;
  }
  :global(.formula .frac) {
    display: inline-flex;
    flex-direction: column;
    text-align: center;
    vertical-align: middle;
    line-height: 1.1;
    margin: 0 0.15em;
  }
  :global(.formula .frac > .num) {
    border-bottom: 1.5px solid currentColor;
    padding: 0 0.25em 0.15em;
  }
  :global(.formula .frac > .den) {
    padding: 0.15em 0.25em 0;
  }
  :global(.formula sup),
  :global(.formula sub) {
    font-size: 0.7em;
    line-height: 0;
  }
</style>
