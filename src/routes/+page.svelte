<script lang="ts">
  import { onMount } from "svelte";
  import { getPredictions, getTeamStats, refreshSchedule } from "$lib/api";
  import type { PredictionsBundle, TeamStats } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns, todayISO, relativeTime, downloadCSV } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let date = $state(todayISO());
  let loading = $state(false);
  let refreshing = $state(false);
  let error = $state<string | null>(null);
  let bundle = $state<PredictionsBundle | null>(null);
  let teamsByName = $state<Map<string, TeamStats>>(new Map());
  let rankByName = $state<Map<string, number>>(new Map());
  let useOptimalExp = $state(true);
  let manualExp = $state(2.0);
  let includePitchers = $state(true);

  async function load() {
    loading = true;
    error = null;
    try {
      const [pred, ts] = await Promise.all([
        getPredictions({
          date,
          exponent: useOptimalExp ? undefined : manualExp,
          includePitchers,
        }),
        getTeamStats({ exponent: useOptimalExp ? undefined : manualExp }),
      ]);
      bundle = pred;
      const byName = new Map<string, TeamStats>();
      for (const t of ts.teams) byName.set(t.team, t);
      teamsByName = byName;
      // Rank teams by Pythagorean win % (descending). 1 = best.
      const ranked = [...ts.teams].sort((a, b) => b.pythag_win_pct - a.pythag_win_pct);
      const ranks = new Map<string, number>();
      ranked.forEach((t, i) => ranks.set(t.team, i + 1));
      rankByName = ranks;
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

  // Tag doubleheaders so a duplicated matchup reads as G1/G2 instead of looking copy-pasted.
  let taggedGames = $derived.by(() => {
    if (!bundle) return [];
    const counts = new Map<string, number>();
    for (const g of bundle.games) {
      const k = `${g.home}|${g.away}`;
      counts.set(k, (counts.get(k) ?? 0) + 1);
    }
    const seen = new Map<string, number>();
    return bundle.games.map((g) => {
      const k = `${g.home}|${g.away}`;
      const total = counts.get(k) ?? 1;
      if (total <= 1) return { ...g, gameTag: "" };
      const idx = (seen.get(k) ?? 0) + 1;
      seen.set(k, idx);
      return { ...g, gameTag: `Game ${idx}` };
    });
  });

  function rpg(t: TeamStats | undefined): string {
    if (!t || t.games_played === 0) return "—";
    return (t.runs_scored / t.games_played).toFixed(1);
  }
  function rapg(t: TeamStats | undefined): string {
    if (!t || t.games_played === 0) return "—";
    return (t.runs_allowed / t.games_played).toFixed(1);
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
    <label class="pitcher-toggle">
      <span class="lbl">
        Pitcher
        <InfoTip text="When on, each team's effective RA blends 60% starter ERA + 40% team RA/G (using the announced probable pitcher). Off = pure team-level Pythagorean." />
      </span>
      <button
        class="toggle"
        class:on={includePitchers}
        role="switch"
        aria-checked={includePitchers}
        onclick={() => { includePitchers = !includePitchers; load(); }}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
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
      <div class="matchup-grid">
        {#each taggedGames as g}
          {@const awayTeam = teamsByName.get(g.away)}
          {@const homeTeam = teamsByName.get(g.home)}
          {@const awayWin = g.awayWinProb >= 0.5}
          {@const homeWin = g.homeWinProb >= 0.5}
          <article class="card matchup">
            {#if g.gameTag}
              <div class="dh-tag">{g.gameTag}</div>
            {/if}

            <div class="grid">
              <!-- AWAY (left) -->
              <div class="side away">
                <h2 class="tname">{g.away}</h2>
                <span class="role">Away</span>
                {#if g.awayPitcher}
                  <div class="pitcher" class:pitcher-faded={!g.awayPitcher.applied}>
                    <span class="pname">{g.awayPitcher.name}</span>
                    {#if g.awayPitcher.inningsPitched > 0}
                      <span class="pera">
                        {g.awayPitcher.era.toFixed(2)} ERA · {g.awayPitcher.gamesStarted} GS
                        {#if !g.awayPitcher.eligibleSample}
                          <span class="pnote">(small sample)</span>
                        {/if}
                      </span>
                    {:else}
                      <span class="pera pnote">no season data yet</span>
                    {/if}
                  </div>
                {:else}
                  <div class="pitcher pitcher-tbd">
                    <span class="pname">Starter TBD</span>
                  </div>
                {/if}
              </div>

              <!-- CENTER: probs, bars, projected runs -->
              <div class="center">
                <div class="probs">
                  <div class="prob" class:winner={awayWin}>
                    {fmtPct(g.awayWinProb, 1)}
                  </div>
                  <div class="prob" class:winner={homeWin}>
                    {fmtPct(g.homeWinProb, 1)}
                  </div>
                </div>
                <div class="bars">
                  <div class="bar bar-away" class:winner={awayWin}>
                    <span class="fill" style="--w: {(g.awayWinProb * 100).toFixed(1)}%"></span>
                  </div>
                  <div class="bar bar-home" class:winner={homeWin}>
                    <span class="fill" style="--w: {(g.homeWinProb * 100).toFixed(1)}%"></span>
                  </div>
                </div>
                <div class="winlabels">
                  <span class="winlabel" class:winner={awayWin}>{g.away.split(" ").pop()} Win</span>
                  <span class="winlabel" class:winner={homeWin}>{g.home.split(" ").pop()} Win</span>
                </div>

                <div class="proj">
                  <div class="proj-num">{fmtRuns(g.awayPredRuns)}</div>
                  <div class="proj-label">Projected Runs</div>
                  <div class="proj-num">{fmtRuns(g.homePredRuns)}</div>
                </div>

                <div class="odds">
                  <span class="odds-val" class:winner={awayWin}>{fmtOdds(g.awayFairOdds)}</span>
                  <span class="odds-label">Fair Odds</span>
                  <span class="odds-val" class:winner={homeWin}>{fmtOdds(g.homeFairOdds)}</span>
                </div>
              </div>

              <!-- HOME (right) -->
              <div class="side home">
                <h2 class="tname">{g.home}</h2>
                <span class="role">Home</span>
                {#if g.homePitcher}
                  <div class="pitcher" class:pitcher-faded={!g.homePitcher.applied}>
                    <span class="pname">{g.homePitcher.name}</span>
                    {#if g.homePitcher.inningsPitched > 0}
                      <span class="pera">
                        {g.homePitcher.era.toFixed(2)} ERA · {g.homePitcher.gamesStarted} GS
                        {#if !g.homePitcher.eligibleSample}
                          <span class="pnote">(small sample)</span>
                        {/if}
                      </span>
                    {:else}
                      <span class="pera pnote">no season data yet</span>
                    {/if}
                  </div>
                {:else}
                  <div class="pitcher pitcher-tbd">
                    <span class="pname">Starter TBD</span>
                  </div>
                {/if}
              </div>

              <!-- AWAY stats (under left) -->
              <div class="stats stats-away">
                <div class="stat">
                  <span class="stat-label">Rank</span>
                  <span class="stat-val">{rankByName.get(g.away) ?? "—"}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">R/G</span>
                  <span class="stat-val">{rpg(awayTeam)}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">RA/G</span>
                  <span class="stat-val">{rapg(awayTeam)}</span>
                </div>
              </div>

              <!-- center spacer (total runs under projected) -->
              <div class="total-line">
                <span class="total-label">Total Runs</span>
                <span class="total-val">{fmtRuns(g.totalRuns)}</span>
              </div>

              <!-- HOME stats (under right) -->
              <div class="stats stats-home">
                <div class="stat">
                  <span class="stat-label">Rank</span>
                  <span class="stat-val">{rankByName.get(g.home) ?? "—"}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">R/G</span>
                  <span class="stat-val">{rpg(homeTeam)}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">RA/G</span>
                  <span class="stat-val">{rapg(homeTeam)}</span>
                </div>
              </div>
            </div>
          </article>
        {/each}
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
    grid-template-columns: auto auto 1fr auto;
    gap: 16px 24px;
    align-items: end;
    margin-bottom: 20px;
  }
  @media (max-width: 900px) {
    .controls {
      grid-template-columns: auto auto auto;
    }
  }
  @media (max-width: 700px) {
    .controls {
      grid-template-columns: 1fr;
    }
  }

  /* Pitcher on/off toggle */
  .pitcher-toggle {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .toggle {
    position: relative;
    width: 76px;
    height: 32px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--bg-soft);
    cursor: pointer;
    padding: 0;
    overflow: hidden;
    font: inherit;
    color: var(--ink-soft);
  }
  .toggle .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--ink-mute);
    transition: transform 0.18s ease, background 0.18s ease;
  }
  .toggle.on .thumb {
    transform: translateX(44px);
    background: var(--good);
  }
  .track-label {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    pointer-events: none;
    transition: opacity 0.18s ease, color 0.18s ease;
  }
  .on-label {
    left: 12px;
    color: var(--good);
    opacity: 0;
  }
  .off-label {
    right: 12px;
    color: var(--ink-mute);
    opacity: 1;
  }
  .toggle.on .on-label { opacity: 1; }
  .toggle.on .off-label { opacity: 0; }
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
  .controls input[type="date"],
  .exp select {
    width: auto;
    height: 38px;
    padding: 0 0.6em;
    box-sizing: border-box;
    line-height: 36px;
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
  .err {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--ink);
  }
  .center {
    text-align: center;
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

  /* ── MATCHUP CARDS ─────────────────────────────────────── */
  .matchup-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 14px;
  }
  .matchup {
    position: relative;
    padding: 22px 26px;
    min-width: 0;
    overflow: hidden;
  }
  .dh-tag {
    position: absolute;
    top: 10px;
    right: 14px;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    border: 1px solid var(--line-soft);
    padding: 2px 8px;
    border-radius: var(--radius-sm);
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.6fr) minmax(0, 1fr);
    grid-template-rows: auto auto;
    gap: 18px 20px;
    align-items: start;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .side.away { align-items: flex-start; text-align: left; }
  .side.home { align-items: flex-end; text-align: right; }
  .tname {
    font-family: var(--serif);
    font-size: 1.5rem;
    line-height: 1.15;
    margin: 0;
    color: var(--ink);
  }
  .role {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--ink-mute);
  }

  .pitcher {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 0.82rem;
    line-height: 1.25;
  }
  .pitcher .pname {
    color: var(--ink);
    font-weight: 500;
  }
  .pitcher .pera {
    font-family: var(--mono);
    color: var(--ink-mute);
    font-size: 0.74rem;
    font-variant-numeric: tabular-nums;
  }
  .pitcher-faded .pname,
  .pitcher-faded .pera { color: var(--ink-mute); }
  .pitcher-tbd .pname { color: var(--ink-mute); font-style: italic; }
  .pnote {
    font-style: italic;
    opacity: 0.75;
  }
  .side.home .pitcher { align-items: flex-end; }

  /* CENTER COLUMN */
  .center {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .probs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    align-items: end;
  }
  .prob {
    font-size: 2rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
    line-height: 1;
  }
  .prob:first-child { text-align: right; }
  .prob:last-child { text-align: left; }
  .prob.winner { color: var(--good); }

  .bars {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .bar {
    height: 14px;
    background: var(--bg-soft);
    border-radius: var(--radius-sm);
    overflow: hidden;
    position: relative;
  }
  .bar .fill {
    display: block;
    height: 100%;
    width: var(--w);
    background: var(--bad);
    transition: width 0.25s ease;
  }
  .bar.winner .fill { background: var(--good); }
  .bar-away .fill {
    margin-left: auto;
  }

  .winlabels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 2px;
  }
  .winlabel {
    font-size: 0.82rem;
    color: var(--ink-mute);
    font-weight: 500;
  }
  .winlabels .winlabel:first-child { text-align: right; }
  .winlabels .winlabel:last-child { text-align: left; }
  .winlabel.winner {
    color: var(--good);
    font-weight: 600;
  }

  .proj {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 14px;
    align-items: center;
    padding: 14px 18px;
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    margin-top: 10px;
  }
  .proj-num {
    font-family: var(--mono);
    font-size: 1.4rem;
    font-weight: 600;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .proj-num:first-child { text-align: right; }
  .proj-num:last-child { text-align: left; }
  .proj-label {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    white-space: nowrap;
  }

  .odds {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 14px;
    align-items: center;
    margin-top: 6px;
    font-size: 0.85rem;
  }
  .odds-val {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
  }
  .odds-val:first-child { text-align: right; }
  .odds-val:last-child { text-align: left; }
  .odds-val.winner { color: var(--good); font-weight: 600; }
  .odds-label {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
  }

  /* STATS ROW (under each side) */
  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    padding-top: 14px;
    border-top: 1px solid var(--line-soft);
  }
  .stats-home { direction: rtl; }
  .stats-home .stat { direction: ltr; }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .stat-label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
  }
  .stat-val {
    font-family: var(--mono);
    font-size: 1rem;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }

  .total-line {
    display: flex;
    justify-content: center;
    align-items: baseline;
    gap: 10px;
    padding-top: 14px;
    border-top: 1px solid var(--line-soft);
  }
  .total-label {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
  }
  .total-val {
    font-family: var(--mono);
    font-size: 1.2rem;
    font-weight: 600;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }

  /* Two cards per row on wider screens */
  @media (min-width: 900px) {
    .matchup-grid {
      grid-template-columns: 1fr 1fr;
    }
  }

  /* Stack columns on narrow screens */
  @media (max-width: 640px) {
    .grid {
      grid-template-columns: 1fr;
      gap: 14px;
    }
    .side.home { align-items: flex-start; text-align: left; }
    .stats-home { direction: ltr; }
  }
</style>
