<script lang="ts">
  import { onMount, tick } from "svelte";

  let { text }: { text: string } = $props();
  let btn: HTMLButtonElement | undefined = $state();
  let box: HTMLSpanElement | undefined = $state();
  let open = $state(false);
  let pos = $state({ top: -9999, left: -9999 });

  function place() {
    if (!open || !btn || !box) return;
    const r = btn.getBoundingClientRect();
    const w = box.offsetWidth;
    const h = box.offsetHeight;
    let left = r.left + r.width / 2 - w / 2;
    let top = r.top - h - 6;
    left = Math.max(8, Math.min(left, window.innerWidth - w - 8));
    if (top < 8) top = r.bottom + 6;
    pos = { top, left };
  }

  function show() {
    open = true;
    tick().then(() => {
      if (open) place();
    });
  }
  function hide() {
    open = false;
  }

  // Overflow on an ancestor (Race table-card uses overflow-x: auto, which
  // also clips the y-axis) would hide a position:absolute tip. Body portal
  // plus position:fixed escapes that.
  onMount(() => {
    const el = box;
    if (!el) return;
    document.body.appendChild(el);
    return () => el.remove();
  });

  $effect(() => {
    if (!open) return;
    const onMove = () => place();
    window.addEventListener("scroll", onMove, true);
    window.addEventListener("resize", onMove);
    return () => {
      window.removeEventListener("scroll", onMove, true);
      window.removeEventListener("resize", onMove);
    };
  });
</script>

<button
  bind:this={btn}
  class="tip"
  type="button"
  aria-label={text}
  onmouseenter={show}
  onmouseleave={hide}
  onfocus={show}
  onblur={hide}
>
  <span aria-hidden="true">ⓘ</span>
</button>
<span
  bind:this={box}
  class="tipbox"
  class:open
  role="tooltip"
  style="top: {pos.top}px; left: {pos.left}px"
>{text}</span>

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
    position: fixed;
    width: 240px;
    padding: 8px 10px;
    background: var(--ink);
    color: var(--bg-elev);
    font-size: 0.78rem;
    font-family: var(--sans);
    font-weight: normal;
    border-radius: 6px;
    box-shadow: var(--shadow);
    z-index: 1000;
    text-align: left;
    line-height: 1.35;
    pointer-events: none;
  }
  .tipbox.open {
    display: block;
  }
</style>
