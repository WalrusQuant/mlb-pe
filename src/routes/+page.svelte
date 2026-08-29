<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getPredictions, getStandings, getTeamStats, refreshSchedule } from "$lib/api";
  import type { Pitcher, PredictionsBundle, TeamStats, TeamStanding } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns, todayISO, relativeTime, downloadCSV } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let date = $state(todayISO());
  let loading = $state(false);
  let refreshing = $state(false);
  let error = $state<string | null>(null);
  let bundle = $state<PredictionsBundle | null>(null);
  let teamsById = $state<Map<number, TeamStats>>(new Map());
  let rankById = $state<Map<number, number>>(new Map());
  let standingByTeamId = $state<Map<number, TeamStanding>>(new Map());
  let useOptimalExp = $state(true);
  let manualExp = $state(2.0);
  let includePitchers = $state(true);
  let includeHomeField = $state(true);
  let includeRecentForm = $state(true);
  let league = $state<"all" | "al" | "nl">("all");
  let sortKey = $state<"time" | "win" | "runs" | "total">("time");
  let sortDir = $state<"asc" | "desc">("asc");

  function pitcherLine(p: Pitcher): string {
    if (p.blendSource === "nextStart" && p.projectedFip != null && p.expectedRuns != null) {
      return `${p.projectedFip.toFixed(2)} FIP · ${p.expectedRuns.toFixed(1)} ER`;
    }
    if (p.inningsPitched > 0) {
      return `${p.era.toFixed(2)} ERA · ${p.gamesStarted} GS`;
    }
    return "no season data yet";
  }

  async function load() {
    loading = true;
    error = null;
    try {
      const [predR, tsR, stR] = await Promise.allSettled([
        getPredictions({
          date,
          exponent: useOptimalExp ? undefined : manualExp,
          includePitchers,
          includeHomeField,
          includeRecentForm,
        }),
        getTeamStats({ exponent: useOptimalExp ? undefined : manualExp }),
        getStandings(),
      ]);
      if (predR.status === "rejected") throw predR.reason;
      bundle = predR.value;

      if (tsR.status === "fulfilled") {
        const ts = tsR.value;
        const byId = new Map<number, TeamStats>();
        for (const t of ts.teams) byId.set(t.teamId, t);
        teamsById = byId;
        const ranked = [...ts.teams].sort((a, b) => b.pythagWinPct - a.pythagWinPct);
        const ranks = new Map<number, number>();
        ranked.forEach((t, i) => ranks.set(t.teamId, i + 1));
        rankById = ranks;
      } else {
        teamsById = new Map();
        rankById = new Map();
      }

      // Standings is W-L decoration + AL/NL filter. Don't take the slate down with it.
      if (stR.status === "fulfilled") {
        const stByTeam = new Map<number, TeamStanding>();
        for (const t of stR.value.teams) stByTeam.set(t.teamId, t);
        standingByTeamId = stByTeam;
      } else {
        standingByTeamId = new Map();
      }
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
    let rows = bundle.games.map((g) => {
      const k = `${g.home}|${g.away}`;
      const total = counts.get(k) ?? 1;
      if (total <= 1) return { ...g, gameTag: "" };
      const idx = (seen.get(k) ?? 0) + 1;
      seen.set(k, idx);
      return { ...g, gameTag: `Game ${idx}` };
    });
    if (league !== "all" && standingByTeamId.size > 0) {
      const want = league === "al" ? 103 : 104;
      rows = rows.filter((g) => standingByTeamId.get(g.homeTeamId)?.leagueId === want);
    }
    const dir = sortDir === "asc" ? 1 : -1;
    rows.sort((a, b) => {
      let c = 0;
      if (sortKey === "time") {
        c = (a.gameDateTime ?? "").localeCompare(b.gameDateTime ?? "") || a.gamePk - b.gamePk;
      } else if (sortKey === "win") {
        c = a.homeWinProb - b.homeWinProb;
      } else if (sortKey === "runs") {
        c = a.homePredRuns - b.homePredRuns;
      } else {
        c = a.totalRuns - b.totalRuns;
      }
      return c * dir;
    });
    return rows;
  });

  function recordFor(teamId: number): string | null {
    const st = standingByTeamId.get(teamId);
    if (!st) return null;
    return `${st.wins}-${st.losses}`;
  }

  function firstPitch(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "—";
    return d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  }

  function toggleSort(key: typeof sortKey) {
    if (sortKey === key) sortDir = sortDir === "asc" ? "desc" : "asc";
    else {
      sortKey = key;
      sortDir = key === "time" ? "asc" : "desc";
    }
  }

  // Link to the game-detail route, carrying the current date + toggle + exponent
  // state in the query string so the breakdown matches this card exactly.
  function gameHref(gamePk: number): string {
    const params = new URLSearchParams({
      date,
      p: String(includePitchers),
      hf: String(includeHomeField),
      rf: String(includeRecentForm),
    });
    if (!useOptimalExp) params.set("exp", String(manualExp));
    return `/game/${gamePk}?${params.toString()}`;
  }

  function rpg(t: TeamStats | undefined): string {
    if (!t || t.gamesPlayed === 0) return "—";
    return (t.runsScored / t.gamesPlayed).toFixed(1);
  }
  function rapg(t: TeamStats | undefined): string {
    if (!t || t.gamesPlayed === 0) return "—";
    return (t.runsAllowed / t.gamesPlayed).toFixed(1);
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
        <InfoTip text="When on, tonight's starter uses the next-start projection (outing ER + remaining innings at team RA/G). Falls back to 60% season ERA + 40% team RA/G if no projection. Off = pure team-level Pythagorean." />
      </span>
      <button
        class="toggle"
        class:on={includePitchers}
        role="switch"
        aria-checked={includePitchers}
        aria-label="Pitcher adjustment"
        onclick={() => { includePitchers = !includePitchers; load(); }}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">
        Home Field
        <InfoTip text="Adds a log-odds shift to home win probability matching MLB's ~54% historical home win rate. The bump shrinks at the extremes (a 90% favorite gains less than a 50/50 game). Off = neutral-site probability." />
      </span>
      <button
        class="toggle"
        class:on={includeHomeField}
        role="switch"
        aria-checked={includeHomeField}
        aria-label="Home field advantage"
        onclick={() => { includeHomeField = !includeHomeField; load(); }}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="pitcher-toggle">
      <span class="lbl">
        Recent Form
        <InfoTip text="When on, each team's RS/G and RA/G are blended 60% season + 40% last-20-games. Captures hot/cold streaks without throwing away the full-season sample. Off = pure season totals." />
      </span>
      <button
        class="toggle"
        class:on={includeRecentForm}
        role="switch"
        aria-checked={includeRecentForm}
        aria-label="Recent form"
        onclick={() => { includeRecentForm = !includeRecentForm; load(); }}
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
      <span class="league">
        <button type="button" class="ghost small" class:on={league === "all"} onclick={() => league = "all"}>All</button>
        <button type="button" class="ghost small" class:on={league === "al"} onclick={() => league = "al"}>AL</button>
        <button type="button" class="ghost small" class:on={league === "nl"} onclick={() => league = "nl"}>NL</button>
      </span>
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
      <div class="slate">
        <table>
          <thead>
            <tr>
              <th><button type="button" class="thb" onclick={() => toggleSort("time")}>Time</button></th>
              <th>Teams</th>
              <th>Pitchers</th>
              <th><button type="button" class="thb" onclick={() => toggleSort("win")}>Win</button></th>
              <th>Fair</th>
              <th><button type="button" class="thb" onclick={() => toggleSort("runs")}>Runs</button></th>
              <th><button type="button" class="thb" onclick={() => toggleSort("total")}>Total</button></th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each taggedGames as g (g.gamePk)}
              {@const awayTeam = teamsById.get(g.awayTeamId)}
              {@const homeTeam = teamsById.get(g.homeTeamId)}
              {@const awayWin = g.awayWinProb >= 0.5}
              {@const homeWin = g.homeWinProb >= 0.5}
              <tr class="gamerow" onclick={() => goto(gameHref(g.gamePk))}>
                <td class="time">
                  <span class="d">{g.date.slice(5)}</span>
                  <span class="t">{firstPitch(g.gameDateTime)}</span>
                  {#if g.gameTag}<span class="dh">{g.gameTag}</span>{/if}
                </td>
                <td class="teams">
                  <div class="tl away">
                    <span class="tn">{g.away}</span>
                    <span class="rec">{recordFor(g.awayTeamId) ?? ""}</span>
                    <span class="mini">#{rankById.get(g.awayTeamId) ?? "—"} · {rpg(awayTeam)} R/G · {rapg(awayTeam)} RA/G</span>
                    {#if g.awayRecent}<span class="mini">L{g.awayRecent.games}: {g.awayRecent.rsPerGame.toFixed(1)}/{g.awayRecent.raPerGame.toFixed(1)}</span>{/if}
                  </div>
                  <div class="tl home">
                    <span class="tn">{g.home}</span>
                    <span class="rec">{recordFor(g.homeTeamId) ?? ""}</span>
                    <span class="mini">#{rankById.get(g.homeTeamId) ?? "—"} · {rpg(homeTeam)} R/G · {rapg(homeTeam)} RA/G</span>
                    {#if g.homeRecent}<span class="mini">L{g.homeRecent.games}: {g.homeRecent.rsPerGame.toFixed(1)}/{g.homeRecent.raPerGame.toFixed(1)}</span>{/if}
                  </div>
                </td>
                <td class="arms">
                  <div class:fade={g.awayPitcher && !g.awayPitcher.applied}>
                    <span class="pn">{g.awayPitcher?.name ?? "TBD"}</span>
                    <span class="ps">{g.awayPitcher ? pitcherLine(g.awayPitcher) : ""}</span>
                  </div>
                  <div class:fade={g.homePitcher && !g.homePitcher.applied}>
                    <span class="pn">{g.homePitcher?.name ?? "TBD"}</span>
                    <span class="ps">{g.homePitcher ? pitcherLine(g.homePitcher) : ""}</span>
                  </div>
                </td>
                <td class="win mono">
                  <span class:good={awayWin} class:bad={!awayWin}>{fmtPct(g.awayWinProb, 1)}</span>
                  <span class:good={homeWin} class:bad={!homeWin}>{fmtPct(g.homeWinProb, 1)}</span>
                </td>
                <td class="fair mono">
                  <span class:good={awayWin}>{fmtOdds(g.awayFairOdds)}</span>
                  <span class:good={homeWin}>{fmtOdds(g.homeFairOdds)}</span>
                </td>
                <td class="runs mono">
                  <span>{fmtRuns(g.awayPredRuns)}</span>
                  <span>{fmtRuns(g.homePredRuns)}</span>
                </td>
                <td class="tot mono">{fmtRuns(g.totalRuns)}</td>
                <td class="go">→</td>
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
    grid-template-columns: auto auto auto auto 1fr auto;
    gap: 16px 24px;
    align-items: end;
    margin-bottom: 20px;
    position: sticky;
    top: 62px;
    z-index: 8;
    background: var(--bg-elev);
  }
  @media (max-width: 1100px) {
    .controls {
      grid-template-columns: auto auto auto auto auto;
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

  .league { display: flex; gap: 4px; margin-left: 8px; }
  .league .on { background: var(--ink); color: var(--bg-elev); border-color: var(--ink); }
  .slate {
    overflow-x: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }
  .slate table { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
  .slate th {
    text-align: left;
    padding: 8px 10px;
    font-size: 0.7rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
    background: var(--bg-elev);
    position: sticky;
    top: 0;
  }
  .thb {
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    text-transform: inherit;
    letter-spacing: inherit;
    padding: 0;
    cursor: pointer;
  }
  .thb:hover { transform: none; color: var(--ink); }
  .slate td {
    padding: 10px;
    border-bottom: 1px solid var(--line-soft);
    vertical-align: top;
  }
  .gamerow { cursor: pointer; }
  .gamerow:hover td { background: var(--bg-soft); }
  .time { white-space: nowrap; min-width: 5.5em; }
  .time .d { display: block; color: var(--ink-mute); font-size: 0.75rem; }
  .time .t { display: block; font-weight: 600; }
  .time .dh { display: block; font-size: 0.68rem; color: var(--ink-mute); text-transform: uppercase; }
  .teams { min-width: 16em; }
  .tl { display: grid; grid-template-columns: 1fr auto; gap: 0 8px; margin-bottom: 8px; }
  .tl:last-child { margin-bottom: 0; }
  .tn { font-weight: 600; color: var(--ink); }
  .rec { font-family: var(--mono); font-size: 0.78rem; color: var(--ink-mute); }
  .mini { grid-column: 1 / -1; font-family: var(--mono); font-size: 0.72rem; color: var(--ink-mute); }
  .arms { min-width: 11em; }
  .arms > div { margin-bottom: 8px; }
  .arms > div:last-child { margin-bottom: 0; }
  .pn { display: block; font-weight: 500; }
  .ps { display: block; font-family: var(--mono); font-size: 0.72rem; color: var(--ink-mute); white-space: nowrap; }
  .fade .pn, .fade .ps { color: var(--ink-mute); }
  .win, .fair, .runs { white-space: nowrap; }
  .win span, .fair span, .runs span { display: block; font-variant-numeric: tabular-nums; }
  .tot { font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap; vertical-align: middle; }
  .go { color: var(--ink-mute); vertical-align: middle; }
  .good { color: var(--good); font-weight: 600; }
  .bad { color: var(--bad); }
</style>
