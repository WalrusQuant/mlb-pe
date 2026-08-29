# Lessons

## Predictions must not die on standings

W-L records come from `/standings`. That call is decoration on the Predictions slate. `Promise.all` with `getStandings()` takes the whole page down on a blip. Load predictions first; standings is best-effort.

## Suspended games duplicate gamePk

Postponed skip is not enough. A suspended-then-resumed game is listed twice as `Final` with the same score (same `gamePk`, often the same `officialDate`). That double-counts runs in team stats and throws Svelte `each_key_duplicate`, which aborts the page and leaves the spinner up — it looks like a hung fetch. Dedupe by `gamePk` in `normalize` / backtest, and never key `{#each}` by `gamePk` alone.

## Historical replay is not a live tracker

The user wants finished games pulled from the schedule, the model run on each, and picks/scores compared to the box score. Do not pitch that as a walk-forward tracking page or as logging predictions going forward. Isolation (stats as of that morning, no same-day leak) is an implementation detail — say "historical replay" / "pull finished games and compare to outcomes."

## Overflow clips InfoTip

`position: absolute` tooltips inside a parent with `overflow-x: auto` get clipped on **both** axes. CSS treats the other axis as `auto` when one is not `visible`. Fix at the component: portal the tip to `document.body` and use `position: fixed` from the trigger’s bounding rect. Do not try to “just use overflow-y: visible.”
