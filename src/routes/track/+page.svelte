<script lang="ts">
  import { onMount } from "svelte";
  import { runBacktest } from "$lib/api";
  import type { BacktestBundle, BacktestGame } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns, downloadCSV } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let includePitchers = $state(true);
  let includeHomeField = $state(true);
  let includeRecentForm = $state(true);
  let includeNbWin = $state(true);
  let includeRateShrink = $state(true);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let bundle = $state<BacktestBundle | null>(null);
  let filter = $state<"all" | "hits" | "misses">("all");

  async function run() {
    loading = true;
    error = null;
    try {
      bundle = await runBacktest({
        includePitchers,
        includeHomeField,
        includeRecentForm,
        includeGameVote: false,
        includePoissonWin: false,
        includeNbWin,
        includeParkFactors: false,
        includeRateShrink,
        includeSplits: false,
        includeBullpen: false,
      });
    } catch (e) {
      error = String(e);
      bundle = null;
    } finally {
      loading = false;
    }
  }

  function flip(which: "p" | "h" | "r" | "n" | "o") {
    if (which === "p") includePitchers = !includePitchers;
    if (which === "h") includeHomeField = !includeHomeField;
    if (which === "r") includeRecentForm = !includeRecentForm;
    if (which === "n") includeNbWin = !includeNbWin;
    if (which === "o") includeRateShrink = !includeRateShrink;
    if (!loading) run();
  }

  function totalError(g: BacktestGame): number {
    return Math.abs(
      g.predHomeRuns + g.predAwayRuns - (g.actualHomeRuns + g.actualAwayRuns),
    );
  }

  function exportCSV() {
    if (!bundle) return;
    downloadCSV(
      `mlb_backtest_${bundle.season}.csv`,
      bundle.games.map((g) => ({
        date: g.date,
        away: g.away,
        home: g.home,
        pick: g.pickedHome ? g.home : g.away,
        winner: g.homeWon ? g.home : g.away,
        hit: g.hit,
        pHome: g.pHome,
        predAway: g.predAwayRuns,
        predHome: g.predHomeRuns,
        actualAway: g.actualAwayRuns,
        actualHome: g.actualHomeRuns,
        totalError: totalError(g),
        marketAwayMl: g.marketAwayMl,
        marketHomeMl: g.marketHomeMl,
        marketPHome: g.marketPHome,
        units: g.units,
        evBetHome: g.evBetHome,
      })),
    );
  }

  let hits = $derived(bundle ? bundle.games.filter((g) => g.hit).length : 0);
  let shown = $derived.by<BacktestGame[]>(() => {
    if (!bundle) return [];
    if (filter === "hits") return bundle.games.filter((g) => g.hit);
    if (filter === "misses") return bundle.games.filter((g) => !g.hit);
    return bundle.games;
  });

  function maxCal(b: BacktestBundle): number {
    return Math.max(0.55, ...b.calibration.flatMap((c) => [c.predicted, c.actual]));
  }

  function fmtUnits(u: number): string {
    const sign = u > 0 ? "+" : "";
    return `${sign}${u.toFixed(1)}u`;
  }

  onMount(run);
</script>

<section>
  <header class="hero">
    <h1>How the model did</h1>
    <p class="subtle">
      Same model as Predictions. Flip a wired switch to A/B against that.
    </p>
  </header>

  <div class="card controls">
    <label class="pitcher-toggle">
      <span class="lbl">
        Pitcher
        <InfoTip text="On = each starter's ERA from their own starts before that day (20 IP minimum). First run fetches starter logs and is slower. Off = team RA/G only." />
      </span>
      <button
        class="toggle"
        class:on={includePitchers}
        role="switch"
        aria-checked={includePitchers}
        aria-label="Pitcher"
        disabled={loading}
        onclick={() => flip("p")}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">Home Field</span>
      <button
        class="toggle"
        class:on={includeHomeField}
        role="switch"
        aria-checked={includeHomeField}
        aria-label="Home field"
        disabled={loading}
        onclick={() => flip("h")}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">Recent Form</span>
      <button
        class="toggle"
        class:on={includeRecentForm}
        role="switch"
        aria-checked={includeRecentForm}
        aria-label="Recent form"
        disabled={loading}
        onclick={() => flip("r")}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">
        Overdisp
        <InfoTip text="Win % from the predicted score (negative binomial, size 2, 40% shrink toward league average). On in live Predictions." />
      </span>
      <button
        class="toggle"
        class:on={includeNbWin}
        role="switch"
        aria-checked={includeNbWin}
        aria-label="Overdispersed"
        disabled={loading}
        onclick={() => flip("n")}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">
        Shrink OS/DS
        <InfoTip text="Pulls OS and DS 40% toward 1.0 before OS×DS×lg. On in live Predictions." />
      </span>
      <button
        class="toggle"
        class:on={includeRateShrink}
        role="switch"
        aria-checked={includeRateShrink}
        aria-label="Shrink OS/DS"
        disabled={loading}
        onclick={() => flip("o")}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <div class="actions">
      <button onclick={run} disabled={loading}>{loading ? "Scoring…" : "Run again"}</button>
      <button class="ghost" onclick={exportCSV} disabled={!bundle || bundle.games.length === 0}>
        Export CSV
      </button>
    </div>
  </div>

  {#if error}
    <div class="card err">
      <strong>Couldn't score historical games.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading && !bundle}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">
        {includePitchers
          ? "Pulling starter logs and closing lines, then scoring every finished game…"
          : "Pulling the season schedule and closing lines…"}
      </p>
    </div>
  {:else if bundle}
    {#if loading}
      <p class="muted scoring">Re-scoring with current toggles…</p>
    {/if}

    <div class="kpis">
      <div class="kpi">
        <span class="sv">{fmtPct(bundle.hitRate, 1)}</span>
        <span class="sl">Right winner</span>
      </div>
      <div class="kpi">
        <span class="sv">{bundle.totalRunsMae.toFixed(2)}</span>
        <span class="sl">Runs MAE</span>
      </div>
      {#if bundle.oddsN > 0}
        <div class="kpi">
          <span class="sv" class:good={bundle.units > 0} class:bad={bundle.units < 0}>{fmtUnits(bundle.units)}</span>
          <span class="sl">+EV P/L</span>
        </div>
        <div class="kpi">
          <span class="sv" class:good={bundle.roi > 0} class:bad={bundle.roi < 0}>{fmtPct(bundle.roi, 1)}</span>
          <span class="sl">ROI · {bundle.unitsN} bets</span>
        </div>
        <div class="kpi">
          <span class="sv">{bundle.oddsN - bundle.unitsN}</span>
          <span class="sl">No bet</span>
        </div>
      {/if}
      <div class="kpi">
        <span class="sv">{bundle.n}</span>
        <span class="sl">Scored</span>
      </div>
      <div class="kpi">
        <span class="sv">{bundle.skippedEarly}</span>
        <span class="sl">Skipped</span>
      </div>
      <div class="kpi">
        <span class="sv">{bundle.brier.toFixed(3)}</span>
        <span class="sl">Brier</span>
      </div>
      <div class="kpi">
        <span class="sv">{bundle.logLoss.toFixed(3)}</span>
        <span class="sl">Log loss</span>
      </div>
    </div>

    <div class="stack">
    <div class="charts">
    {#if bundle.calibration.length > 0}
      {@const mx = maxCal(bundle)}
      <div class="card">
        <h2>When we said X%, how often did they win?</h2>
        <div class="cal" role="img" aria-label="Calibration">
          {#each bundle.calibration as c}
            <div class="cal-col">
              <div class="cal-bars">
                <span class="bar pred" style="height: {(c.predicted / mx) * 100}%"></span>
                <span class="bar act" style="height: {(c.actual / mx) * 100}%"></span>
              </div>
              <span class="cal-l">{c.label}</span>
              <span class="cal-n">n={c.n}</span>
            </div>
          {/each}
        </div>
        <p class="muted small">Gray = model. Green = actual.</p>
      </div>
    {/if}

    {#if bundle.monthly.length > 0}
      <div class="card">
        <h2>By month</h2>
        <table class="months">
          <thead>
            <tr>
              <th>Month</th><th>N</th><th>Right winner</th><th>Brier</th><th>Runs MAE</th>
            </tr>
          </thead>
          <tbody>
            {#each bundle.monthly as m}
              <tr>
                <td>{m.month}</td>
                <td class="mono">{m.n}</td>
                <td class="mono">{fmtPct(m.hitRate, 1)}</td>
                <td class="mono">{m.brier.toFixed(3)}</td>
                <td class="mono">{m.totalRunsMae.toFixed(2)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    </div>

    <div class="card games-card">
      <div class="games-head">
        <h2>Every game</h2>
        <span class="league">
          <button type="button" class="ghost small" class:on={filter === "all"} onclick={() => filter = "all"}>All</button>
          <button type="button" class="ghost small" class:on={filter === "hits"} onclick={() => filter = "hits"}>Hits</button>
          <button type="button" class="ghost small" class:on={filter === "misses"} onclick={() => filter = "misses"}>Misses</button>
        </span>
      </div>
      <div class="slate">
        <table class="games">
          <colgroup>
            <col class="c-date" />
            <col class="c-matchup" />
            <col class="c-team" />
            <col class="c-team" />
            <col class="c-num" />
            <col class="c-num" />
            <col class="c-num" />
            <col class="c-score" />
            <col class="c-score" />
            <col class="c-num" />
          </colgroup>
          <thead>
            <tr>
              <th>Date</th>
              <th>Matchup</th>
              <th>Pick</th>
              <th>Winner</th>
              <th>p(home)</th>
              <th>Close</th>
              <th>P/L</th>
              <th>Pred</th>
              <th>Final</th>
              <th>Runs miss</th>
            </tr>
          </thead>
          <tbody>
            {#each shown as g, i (`${g.gamePk}-${i}`)}
              <tr class:miss={!g.hit}>
                <td class="mono">{g.date}</td>
                <td>{g.away} @ {g.home}</td>
                <td>{g.pickedHome ? g.home : g.away}</td>
                <td>
                  {g.homeWon ? g.home : g.away}
                  {#if g.hit}<span class="ok"> hit</span>{:else}<span class="no"> miss</span>{/if}
                </td>
                <td class="mono num">{fmtPct(g.pHome, 1)}</td>
                <td class="mono num">
                  {#if g.marketPHome != null}
                    {fmtPct(g.marketPHome, 1)}
                    <span class="muted">{fmtOdds(g.marketHomeMl ?? 0)}</span>
                  {:else}—{/if}
                </td>
                <td class="mono num" class:good={(g.units ?? 0) > 0} class:bad={(g.units ?? 0) < 0}>
                  {g.units != null ? fmtUnits(g.units) : "—"}
                </td>
                <td class="mono num">{fmtRuns(g.predAwayRuns)}–{fmtRuns(g.predHomeRuns)}</td>
                <td class="mono num">{g.actualAwayRuns}–{g.actualHomeRuns}</td>
                <td class="mono num">{totalError(g).toFixed(1)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
    </div>
  {/if}
</section>

<style>
  .hero { margin-bottom: 20px; }
  .hero p { max-width: 70ch; }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 18px 28px;
    align-items: end;
    margin-bottom: 16px;
  }
  .pitcher-toggle { display: flex; flex-direction: column; gap: 6px; }
  .lbl { font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-mute); }
  .toggle {
    position: relative;
    width: 76px; height: 32px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--bg-soft);
    padding: 0;
  }
  .toggle:hover { transform: none; }
  .toggle:disabled { opacity: 0.55; }
  .thumb {
    position: absolute; top: 3px; left: 3px; width: 24px; height: 24px;
    border-radius: 50%; background: var(--ink-mute);
    transition: left 0.18s ease, background 0.18s ease;
  }
  .toggle.on { background: var(--good-soft); border-color: var(--good); }
  .toggle.on .thumb { left: 47px; background: var(--good); }
  .track-label {
    position: absolute; top: 50%; transform: translateY(-50%);
    font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em;
    pointer-events: none;
  }
  .on-label { left: 12px; color: var(--good); opacity: 0; }
  .off-label { right: 12px; color: var(--ink-mute); opacity: 1; }
  .toggle.on .on-label { opacity: 1; }
  .toggle.on .off-label { opacity: 0; }
  .actions { display: flex; gap: 8px; align-items: center; margin-left: auto; }
  .err { border-color: var(--accent); background: var(--accent-soft); }
  .center { text-align: center; padding: 36px 20px; }
  .small { font-size: 0.82rem; }
  .scoring { margin: 0 0 10px; }
  .spinner {
    display: inline-block; width: 18px; height: 18px;
    border: 2px solid var(--line); border-top-color: var(--accent);
    border-radius: 50%; animation: spin 0.8s linear infinite; margin-right: 8px;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .kpis {
    display: grid;
    grid-template-columns: repeat(9, minmax(0, 1fr));
    gap: 8px;
    margin: 12px 0 14px;
  }
  .kpis:not(:has(.kpi:nth-child(9))) {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }
  .kpi {
    background: var(--bg-elev);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    min-width: 0;
  }
  .sv { display: block; font-family: var(--mono); font-size: 1.05rem; font-weight: 600; line-height: 1.2; }
  .sv.good { color: var(--good); }
  .sv.bad { color: var(--bad); }
  td.good { color: var(--good); }
  td.bad { color: var(--bad); }
  .sl {
    display: block;
    margin-top: 2px;
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-mute);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .charts {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: 12px;
    align-items: stretch;
  }
  @media (max-width: 1100px) {
    .kpis, .kpis:not(:has(.kpi:nth-child(9))) { grid-template-columns: repeat(3, minmax(0, 1fr)); }
    .charts { grid-template-columns: 1fr; }
  }
  .stack { display: flex; flex-direction: column; gap: 12px; }
  .card h2 { font-size: 1.1rem; margin: 0 0 8px; }
  .cal {
    display: flex;
    gap: 6px;
    align-items: stretch;
    width: 100%;
    margin: 8px 0 4px;
  }
  .cal-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .cal-bars {
    display: flex;
    gap: 3px;
    align-items: flex-end;
    justify-content: center;
    height: 120px;
    width: 100%;
  }
  .bar {
    flex: 1;
    max-width: 22px;
    min-width: 6px;
    border-radius: 2px 2px 0 0;
  }
  .bar.pred { background: var(--ink-mute); }
  .bar.act { background: var(--good); }
  .cal-l, .cal-n {
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-mute);
    white-space: nowrap;
  }
  .games-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }
  .games-head h2 { margin: 0; }
  .league { display: flex; gap: 4px; }
  .league .on { background: var(--ink); color: var(--bg-elev); border-color: var(--ink); }
  .games-card { padding-bottom: 12px; }
  .slate { overflow: auto; max-height: 70vh; }
  .months {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.86rem;
  }
  .months th, .months td {
    text-align: left;
    padding: 6px 12px 6px 0;
    border-bottom: 1px solid var(--line-soft);
    white-space: nowrap;
  }
  .months th {
    font-size: 0.7rem; font-weight: 500;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-mute);
    border-bottom: 1px solid var(--line);
  }
  .months th:last-child, .months td:last-child { padding-right: 0; }
  .games {
    width: 100%;
    table-layout: fixed;
    border-collapse: collapse;
    font-size: 0.86rem;
  }
  .c-date { width: 7.2rem; }
  .c-matchup { width: auto; }
  .c-team { width: 14rem; }
  .c-num { width: 6.2rem; }
  .c-score { width: 7rem; }
  .games th, .games td {
    text-align: left;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line-soft);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .games th {
    font-size: 0.7rem; font-weight: 500;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-mute);
    border-bottom: 1px solid var(--line);
    position: sticky; top: 0; background: var(--bg-elev);
  }
  .games .num { font-variant-numeric: tabular-nums; }
  .miss td { color: var(--ink-soft); }
  .ok { color: var(--good); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; }
  .no { color: var(--bad); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; }
</style>
