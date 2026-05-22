<script lang="ts">
  import { onMount } from "svelte";
  import { getTeamStats } from "$lib/api";
  import type { TeamStats } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let loading = $state(true);
  let error = $state<string | null>(null);
  let teams = $state<TeamStats[]>([]);
  let leagueAvgRuns = $state(4.5);
  let optimalExponent = $state(2.0);

  // Editable matchup state
  let homeId = $state<number | null>(null);
  let awayId = $state<number | null>(null);
  let homeRS = $state(540);
  let homeRA = $state(410);
  let homeG = $state(100);
  let awayRS = $state(400);
  let awayRA = $state(510);
  let awayG = $state(99);
  let exponent = $state(1.83);

  // Local math — mirrored from src-tauri/src/model.rs so slider interaction stays snappy.
  function pythag(rs: number, ra: number, x: number): number {
    if (rs <= 0 && ra <= 0) return 0.5;
    const num = Math.pow(rs, x);
    return num / (num + Math.pow(ra, x));
  }
  function log5(pA: number, pB: number): number {
    const num = pA * (1 - pB);
    const denom = num + (1 - pA) * pB;
    return denom === 0 ? 0.5 : num / denom;
  }
  function americanOdds(p: number): number {
    if (p <= 0 || p >= 1) return 0;
    return p > 0.5
      ? Math.round((-100 * p) / (1 - p))
      : Math.round(((1 - p) * 100) / p);
  }

  // Derived predictions update instantly when any input changes.
  let result = $derived.by(() => {
    const pHome = pythag(homeRS, homeRA, exponent);
    const pAway = pythag(awayRS, awayRA, exponent);
    const homeWin = log5(pHome, pAway);
    const awayWin = 1 - homeWin;
    const osH = homeG > 0 ? homeRS / homeG / leagueAvgRuns : 0;
    const dsH = homeG > 0 ? homeRA / homeG / leagueAvgRuns : 0;
    const osA = awayG > 0 ? awayRS / awayG / leagueAvgRuns : 0;
    const dsA = awayG > 0 ? awayRA / awayG / leagueAvgRuns : 0;
    const eHome = osH * dsA * leagueAvgRuns;
    const eAway = osA * dsH * leagueAvgRuns;
    return {
      pHome,
      pAway,
      homeWin,
      awayWin,
      homeFairOdds: americanOdds(homeWin),
      awayFairOdds: americanOdds(awayWin),
      eHome,
      eAway,
      total: eHome + eAway,
      osH,
      dsH,
      osA,
      dsA,
    };
  });

  // Sensitivity chart: home log5 win prob vs exponent across [0.5, 4.0]
  let curve = $derived.by(() => {
    const points: { x: number; y: number }[] = [];
    const steps = 80;
    const min = 0.5;
    const max = 4.0;
    for (let i = 0; i < steps; i++) {
      const e = min + ((max - min) * i) / (steps - 1);
      const ph = pythag(homeRS, homeRA, e);
      const pa = pythag(awayRS, awayRA, e);
      points.push({ x: e, y: log5(ph, pa) });
    }
    return { points, min, max };
  });

  async function load() {
    loading = true;
    error = null;
    try {
      const bundle = await getTeamStats();
      teams = bundle.teams;
      leagueAvgRuns = bundle.leagueAvgRuns;
      optimalExponent = bundle.optimalExponent;
      exponent = optimalExponent || 1.83;
      if (teams.length >= 2) {
        homeId = teams[0].team_id;
        awayId = teams[1].team_id;
        applyTeam("home", teams[0]);
        applyTeam("away", teams[1]);
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function applyTeam(side: "home" | "away", t: TeamStats) {
    if (side === "home") {
      homeRS = t.runs_scored;
      homeRA = t.runs_allowed;
      homeG = t.games_played;
    } else {
      awayRS = t.runs_scored;
      awayRA = t.runs_allowed;
      awayG = t.games_played;
    }
  }

  function onHomeSelect(e: Event) {
    const id = parseInt((e.target as HTMLSelectElement).value);
    homeId = id;
    const t = teams.find((x) => x.team_id === id);
    if (t) applyTeam("home", t);
  }
  function onAwaySelect(e: Event) {
    const id = parseInt((e.target as HTMLSelectElement).value);
    awayId = id;
    const t = teams.find((x) => x.team_id === id);
    if (t) applyTeam("away", t);
  }

  function resetTeam(side: "home" | "away") {
    const id = side === "home" ? homeId : awayId;
    const t = teams.find((x) => x.team_id === id);
    if (t) applyTeam(side, t);
  }

  function resetExponent() {
    exponent = optimalExponent || 1.83;
  }

  onMount(load);

  // Chart geometry
  const CW = 480;
  const CH = 140;
  const PX = 28;
  const PY = 14;
  let pathD = $derived.by(() => {
    const { points, min, max } = curve;
    if (points.length === 0) return "";
    const xScale = (x: number) =>
      PX + ((CW - PX * 2) * (x - min)) / (max - min);
    const yScale = (y: number) => CH - PY - (CH - PY * 2) * y;
    return points
      .map((p, i) => `${i === 0 ? "M" : "L"}${xScale(p.x).toFixed(1)} ${yScale(p.y).toFixed(1)}`)
      .join(" ");
  });
  let markerX = $derived(
    PX + ((CW - PX * 2) * (exponent - 0.5)) / (4.0 - 0.5),
  );
  let markerY = $derived(CH - PY - (CH - PY * 2) * result.homeWin);
</script>

<section>
  <header class="hero">
    <h1>Playground</h1>
    <p class="subtle">
      Drag the exponent. Change a team's runs. Watch the win % move. The math is the same one
      that powers the predictions tab — but here you control all the inputs.
    </p>
  </header>

  {#if error}
    <div class="card err">
      <strong>Couldn't load team data.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Loading team stats…</p>
    </div>
  {:else}
    <div class="grid">
      <!-- HOME -->
      <div class="card team">
        <header class="teamhdr">
          <span class="lbl">Home</span>
          <select value={homeId} onchange={onHomeSelect}>
            {#each teams as t}
              <option value={t.team_id}>{t.team}</option>
            {/each}
          </select>
        </header>
        <div class="fields">
          <label>
            <span>Runs Scored</span>
            <input type="number" min="0" bind:value={homeRS} />
          </label>
          <label>
            <span>Runs Allowed</span>
            <input type="number" min="0" bind:value={homeRA} />
          </label>
          <label>
            <span>Games</span>
            <input type="number" min="1" bind:value={homeG} />
          </label>
        </div>
        <p class="readouts mono">
          Pythag W%: <strong>{fmtPct(result.pHome, 1)}</strong>
          &nbsp;·&nbsp; OS: {result.osH.toFixed(2)}
          &nbsp;·&nbsp; DS: {result.dsH.toFixed(2)}
        </p>
        <button class="ghost small" onclick={() => resetTeam("home")}>Reset to actuals</button>
      </div>

      <!-- AWAY -->
      <div class="card team">
        <header class="teamhdr">
          <span class="lbl">Away</span>
          <select value={awayId} onchange={onAwaySelect}>
            {#each teams as t}
              <option value={t.team_id}>{t.team}</option>
            {/each}
          </select>
        </header>
        <div class="fields">
          <label>
            <span>Runs Scored</span>
            <input type="number" min="0" bind:value={awayRS} />
          </label>
          <label>
            <span>Runs Allowed</span>
            <input type="number" min="0" bind:value={awayRA} />
          </label>
          <label>
            <span>Games</span>
            <input type="number" min="1" bind:value={awayG} />
          </label>
        </div>
        <p class="readouts mono">
          Pythag W%: <strong>{fmtPct(result.pAway, 1)}</strong>
          &nbsp;·&nbsp; OS: {result.osA.toFixed(2)}
          &nbsp;·&nbsp; DS: {result.dsA.toFixed(2)}
        </p>
        <button class="ghost small" onclick={() => resetTeam("away")}>Reset to actuals</button>
      </div>
    </div>

    <!-- EXPONENT SLIDER -->
    <div class="card slider-card">
      <header class="sliderhdr">
        <div>
          <span class="lbl">Pythagorean Exponent <InfoTip text="The power in W% = RS^x / (RS^x + RA^x). Bill James used 2; modern fits hover near 1.83." /></span>
          <h2>{exponent.toFixed(2)}</h2>
        </div>
        <div class="rightcol">
          <button class="ghost small" onclick={resetExponent}>
            Use season-optimal ({optimalExponent.toFixed(2)})
          </button>
        </div>
      </header>
      <input
        type="range"
        min="0.5"
        max="4.0"
        step="0.01"
        bind:value={exponent}
        class="range"
      />
      <div class="ticks">
        <span>0.5</span><span>1.0</span><span>1.5</span><span>2.0</span><span>2.5</span><span>3.0</span><span>3.5</span><span>4.0</span>
      </div>
    </div>

    <!-- LIVE RESULTS -->
    <div class="card result-card">
      <div class="result-grid">
        <div class="result-block">
          <span class="lbl">Home Win %</span>
          <div class="big" class:fav={result.homeWin >= 0.5}>{fmtPct(result.homeWin, 1)}</div>
          <span class="mono">{fmtOdds(result.homeFairOdds)} fair</span>
        </div>
        <div class="result-block">
          <span class="lbl">Away Win %</span>
          <div class="big" class:fav={result.awayWin > 0.5}>{fmtPct(result.awayWin, 1)}</div>
          <span class="mono">{fmtOdds(result.awayFairOdds)} fair</span>
        </div>
        <div class="result-block">
          <span class="lbl">Predicted Home Runs</span>
          <div class="big">{fmtRuns(result.eHome)}</div>
        </div>
        <div class="result-block">
          <span class="lbl">Predicted Away Runs</span>
          <div class="big">{fmtRuns(result.eAway)}</div>
        </div>
        <div class="result-block">
          <span class="lbl">Total Runs</span>
          <div class="big">{fmtRuns(result.total)}</div>
        </div>
        <div class="result-block">
          <span class="lbl">League Avg Runs</span>
          <input type="number" step="0.05" min="3" max="6" bind:value={leagueAvgRuns} class="lgavg" />
          <span class="subtle small">per team / game</span>
        </div>
      </div>
    </div>

    <!-- SENSITIVITY CHART -->
    <div class="card chart-card">
      <h3>Sensitivity: home win % vs. exponent</h3>
      <p class="subtle small">
        How much does the prediction shift as we vary the exponent across [0.5, 4.0]?
        The vertical line is your current value.
      </p>
      <svg
        viewBox={`0 0 ${CW} ${CH}`}
        preserveAspectRatio="none"
        class="chart"
        aria-label="Home win probability sensitivity"
      >
        <!-- 50% line -->
        <line
          x1={PX}
          x2={CW - PX}
          y1={CH - PY - (CH - PY * 2) * 0.5}
          y2={CH - PY - (CH - PY * 2) * 0.5}
          class="axis-mid"
        />
        <!-- curve -->
        <path d={pathD} class="curve" />
        <!-- current exponent marker -->
        <line x1={markerX} x2={markerX} y1={PY} y2={CH - PY} class="marker-x" />
        <circle cx={markerX} cy={markerY} r="4" class="marker-pt" />
        <!-- y-axis labels -->
        <text x="4" y={PY + 6} class="tick">100%</text>
        <text x="4" y={CH - PY + 4} class="tick">0%</text>
        <text x="4" y={CH - PY - (CH - PY * 2) * 0.5 + 4} class="tick">50%</text>
        <!-- x-axis labels -->
        <text x={PX} y={CH - 1} class="tick">0.5</text>
        <text x={PX + (CW - PX * 2) * (2.0 - 0.5) / (4 - 0.5)} y={CH - 1} class="tick" text-anchor="middle">2.0</text>
        <text x={CW - PX} y={CH - 1} class="tick" text-anchor="end">4.0</text>
      </svg>
    </div>
  {/if}
</section>

<style>
  .hero {
    margin-bottom: 20px;
  }
  .hero p {
    max-width: 64ch;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-bottom: 16px;
  }
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
  .team {
    padding: 16px 18px;
  }
  .teamhdr {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .teamhdr select {
    flex: 1;
    font-weight: 600;
    font-size: 1rem;
  }
  .lbl {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 10px;
    margin-bottom: 10px;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.78rem;
    color: var(--ink-soft);
  }
  .readouts {
    color: var(--ink-soft);
    font-size: 0.85rem;
    margin: 6px 0 10px;
  }
  .small {
    font-size: 0.82rem;
  }
  .slider-card {
    margin-bottom: 16px;
  }
  .sliderhdr {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .sliderhdr h2 {
    font-family: var(--mono);
    font-size: 2rem;
    margin: 4px 0 0;
  }
  .rightcol {
    text-align: right;
  }
  .range {
    width: 100%;
    accent-color: var(--accent);
  }
  .ticks {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-mute);
  }
  .result-card {
    margin-bottom: 16px;
  }
  .result-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
  }
  @media (max-width: 700px) {
    .result-grid {
      grid-template-columns: 1fr 1fr;
    }
  }
  .result-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .result-block .big {
    font-family: var(--serif);
    font-size: 1.7rem;
    font-weight: 600;
  }
  .result-block .big.fav {
    color: var(--good);
  }
  .lgavg {
    max-width: 80px;
    margin-top: 2px;
  }
  .chart-card {
    padding-bottom: 6px;
  }
  .chart {
    width: 100%;
    height: 170px;
  }
  .axis-mid {
    stroke: var(--line);
    stroke-dasharray: 3 4;
    stroke-width: 1;
  }
  .curve {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }
  .marker-x {
    stroke: var(--ink-mute);
    stroke-width: 1;
    stroke-dasharray: 2 3;
  }
  .marker-pt {
    fill: var(--accent);
    stroke: var(--bg-elev);
    stroke-width: 2;
  }
  .tick {
    font-family: var(--mono);
    font-size: 9px;
    fill: var(--ink-mute);
  }
  .center {
    text-align: center;
    padding: 40px 20px;
  }
  .err {
    border-color: var(--accent);
    background: var(--accent-soft);
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
