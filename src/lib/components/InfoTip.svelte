<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<button class="tip" type="button" aria-label={text}>
  <span aria-hidden="true">ⓘ</span>
  <span class="tipbox">{text}</span>
</button>

<style>
  .tip {
    display: inline-flex;
    align-items: center;
    color: var(--ink-mute);
    font-size: 0.85em;
    margin-left: 0.3em;
    position: relative;
    cursor: help;
    background: transparent;
    border: none;
    padding: 0;
    transform: none;
  }
  .tip:hover,
  .tip:focus-visible {
    color: var(--ink);
    outline: none;
    transform: none;
  }
  .tipbox {
    display: none;
    position: absolute;
    bottom: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
    width: 240px;
    padding: 8px 10px;
    background: var(--ink);
    color: var(--bg-elev);
    font-size: 0.78rem;
    font-family: var(--sans);
    font-weight: normal;
    border-radius: 6px;
    box-shadow: var(--shadow);
    z-index: 50;
    text-align: left;
    line-height: 1.35;
    pointer-events: none;
  }
  .tip:hover .tipbox,
  .tip:focus-visible .tipbox {
    display: block;
  }
</style>
