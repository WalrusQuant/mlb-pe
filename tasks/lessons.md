# Lessons

## Predictions must not die on standings

W-L records come from `/standings`. That call is decoration on the Predictions slate. `Promise.all` with `getStandings()` takes the whole page down on a blip. Load predictions first; standings is best-effort.

## Suspended games duplicate gamePk

Postponed skip is not enough. A suspended-then-resumed game is listed twice as `Final` with the same score (same `gamePk`, often the same `officialDate`). That double-counts runs in team stats and throws Svelte `each_key_duplicate`, which aborts the page and leaves the spinner up — it looks like a hung fetch. Dedupe by `gamePk` in `normalize` / backtest, and never key `{#each}` by `gamePk` alone.

## Historical replay is not a live tracker

The user wants finished games pulled from the schedule, the model run on each, and picks/scores compared to the box score. Do not pitch that as a walk-forward tracking page or as logging predictions going forward. Isolation (stats as of that morning, no same-day leak) is an implementation detail — say "historical replay" / "pull finished games and compare to outcomes."

## Track toggles = wired factors (+ the one under test)

The user said this repeatedly: Track may only show toggles for factors that are **in the live model**. That is Pitcher, Home Field, Recent Form, Overdisp, Shrink OS/DS — all default On. Failed experiments (Game vote, Poisson, Park) do not stay on the bar. The next untested lever can be one extra switch, default Off, so it can actually be A/B'd against a known baseline. Home/road: 13 fewer hits, Brier worse — do not promote. Bullpen leftover-vs-team-RA: noise, do not promote. Failed tests come off the bar. Do not interpret this as "only three switches" or as "keep the whole lab bar." Never put win % back through Pythagorean unless asked.

## Overflow clips InfoTip

`position: absolute` tooltips inside a parent with `overflow-x: auto` get clipped on **both** axes. CSS treats the other axis as `auto` when one is not `visible`. Fix at the component: portal the tip to `document.body` and use `position: fixed` from the trigger’s bounding rect. Do not try to “just use overflow-y: visible.”
