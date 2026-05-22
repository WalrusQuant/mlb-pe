<script lang="ts">
  import { onMount } from "svelte";
  import { getPredictions, refreshSchedule } from "$lib/api";
  import type { PredictionsBundle } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns, todayISO, relativeTime, downloadCSV } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let date = $state(todayISO());
  let loading = $state(false);
  let refreshing = $state(false);
  let error = $state<string | null>(null);
  let bundle = $state<PredictionsBundle | null>(null);
  let useOptimalExp = $state(true);
  let manualExp = $state(2.0);

  async function load() {
    loading = true;
    error = null;
    try {
      bundle = await getPredictions({
        date,
        exponent: useOptimalExp ? undefined : manualExp,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    refreshing = true;
    error = null;
    try {
      await refreshSchedule();
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      refreshing = false;
    }
  }

  function exportCSV() {
    if (!bundle || bundle.games.length === 0) return;
    const rows = bundle.games.map((g) => ({
      Date: g.date,
      Home: g.home,
      Away: g.away,
      Home_Win_Probability: g.homeWinProb,
      Home_Fair_Odds: g.homeFairOdds,
      Away_Win_Probability: g.awayWinProb,
      Away_Fair_Odds: g.awayFairOdds,
      Home_Predicted_Runs: g.homePredRuns,
      Away_Predicted_Runs: g.awayPredRuns,
      Total_Runs: g.totalRuns,
    }));
    downloadCSV(`mlb_predictions_${bundle.date}.csv`, rows);
  }

  function jumpToNextAvailable() {
    if (!bundle || bundle.availableDates.length === 0) return;
    const after = bundle.availableDates.find((d) => d >= todayISO()) ?? bundle.availableDates[0];
    date = after;
    load();
  }

  onMount(load);
</script>

<section>
  <header class="hero">
    <div>
      <h1>Today's MLB Predictions</h1>
      <p class="subtle">
        Each game's win probability and predicted runs, derived from team-level Pythagorean
        expectation and log5. The exponent is fit to this season's actual results.
      </p>
    </div>
  </header>

  <div class="card controls">
    <label>
      <span class="lbl">Date</span>
      <input type="date" bind:value={date} onchange={load} />
    </label>
    <label class="exp">
      <span class="lbl">
        Exponent
        <InfoTip text="The power in W% = RS^x / (RS^x + RA^x). Default is optimized to minimize MSE against this season's actual win %." />
      </span>
      <div class="exprow">
        <select
          value={useOptimalExp ? "optimal" : "manual"}
          onchange={(e) => {
            useOptimalExp = (e.currentTarget as HTMLSelectElement).value === "optimal";
            load();
          }}
        >
          <option value="optimal">Optimized for season</option>
          <option value="manual">Manual</option>
        </select>
        {#if !useOptimalExp}
          <input
            type="number"
            min="0.5"
            max="5"
            step="0.05"
            bind:value={manualExp}
            onchange={load}
          />
        {/if}
      </div>
    </label>
    <div class="actions">
      <button class="ghost" onclick={refresh} disabled={refreshing || loading}>
        {refreshing ? "Refreshing…" : "Refresh data"}
      </button>
      <button onclick={exportCSV} disabled={!bundle || bundle.games.length === 0}>
        Export CSV
      </button>
    </div>
  </div>

  {#if error}
    <div class="card err">
      <strong>Couldn't load predictions.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading && !bundle}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Pulling season schedule from MLB Stats API…</p>
    </div>
  {:else if bundle}
    <div class="meta">
      <span class="badge">Season {bundle.season}</span>
      <span class="badge">x = {bundle.exponent.toFixed(3)}</span>
      <span class="badge">League avg: {bundle.leagueAvgRuns.toFixed(2)} R/team/g</span>
      <span class="badge">Updated {relativeTime(bundle.lastUpdated)}</span>
    </div>

    {#if bundle.games.length === 0}
      <div class="card empty">
        <h3>No predictions for {bundle.date}</h3>
        {#if bundle.availableDates.length > 0}
          <p>
            Next scheduled game date:
            <button class="ghost" onclick={jumpToNextAvailable}>
              {bundle.availableDates.find((d) => d >= todayISO()) ?? bundle.availableDates[0]}
            </button>
          </p>
        {:else}
          <p class="muted">
            No regular-season games found. The season may not have started yet, or has ended.
          </p>
        {/if}
      </div>
    {:else}
      <div class="card table-card">
        <table>
          <thead>
            <tr>
              <th>Matchup</th>
              <th>Home Win<InfoTip text="P(home wins) via log5 of team Pythagorean win %." /></th>
              <th>Fair Home<InfoTip text="The 'no-vig' American odds implied by the win probability." /></th>
              <th>Away Win</th>
              <th>Fair Away</th>
              <th>H Runs<InfoTip text="OS_home × DS_away × league-avg runs." /></th>
              <th>A Runs</th>
              <th>Total</th>
            </tr>
          </thead>
          <tbody>
            {#each bundle.games as g (g.home + g.away)}
              {@const homeFav = g.homeWinProb >= 0.5}
              <tr>
                <td class="matchup">
                  <div class="away muted">{g.away}</div>
                  <div class="at">@</div>
                  <div class="home">{g.home}</div>
                </td>
                <td class:fav={homeFav}>{fmtPct(g.homeWinProb)}</td>
                <td class="mono">{fmtOdds(g.homeFairOdds)}</td>
                <td class:fav={!homeFav}>{fmtPct(g.awayWinProb)}</td>
                <td class="mono">{fmtOdds(g.awayFairOdds)}</td>
                <td>{fmtRuns(g.homePredRuns)}</td>
                <td>{fmtRuns(g.awayPredRuns)}</td>
                <td class="strong">{fmtRuns(g.totalRuns)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if bundle.skipped.length > 0}
      <p class="subtle small">
        Skipped (missing season stats): {bundle.skipped.join(", ")}
      </p>
    {/if}
  {/if}
</section>

<style>
  .hero {
    margin-bottom: 22px;
  }
  .hero p {
    max-width: 60ch;
  }
  .controls {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 16px 24px;
    align-items: end;
    margin-bottom: 20px;
  }
  @media (max-width: 700px) {
    .controls {
      grid-template-columns: 1fr;
    }
  }
  .lbl {
    display: block;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
    margin-bottom: 6px;
  }
  .exprow {
    display: flex;
    gap: 8px;
  }
  .exp select {
    min-width: 200px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 14px 0 14px;
  }
  .table-card {
    padding: 6px 6px;
  }
  .matchup {
    line-height: 1.25;
    font-size: 0.95rem;
  }
  .matchup .at {
    color: var(--ink-mute);
    font-size: 0.8em;
    margin: 1px 0;
  }
  .matchup .home {
    font-weight: 600;
  }
  td.fav {
    color: var(--good);
    font-weight: 600;
  }
  td.strong {
    font-weight: 600;
  }
  td.mono {
    font-family: var(--mono);
  }
  .err {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--ink);
  }
  .center {
    text-align: center;
    padding: 40px 20px;
  }
  .empty {
    text-align: center;
    padding: 36px 20px;
  }
  .small {
    font-size: 0.82rem;
  }
  .spinner {
    display: inline-block;
    width: 18px;
    height: 18px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: middle;
    margin-right: 8px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
