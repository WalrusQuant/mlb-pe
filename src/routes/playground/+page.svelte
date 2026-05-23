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
  // Optional starting-pitcher overrides. null = no adjustment (pure pythag).
  let homePitcherERA = $state<number | null>(null);
  let homePitcherIP = $state<number | null>(null);
  let awayPitcherERA = $state<number | null>(null);
  let awayPitcherIP = $state<number | null>(null);
  // Master toggle: when off, pitcher inputs are kept but ignored by the math.
  let applyPitchers = $state(true);
  // Home-field advantage toggle.
  let applyHomeField = $state(true);

  // Mirrors src-tauri/src/model.rs constants.
  const STARTER_SHARE = 0.6;
  const MIN_IP_FOR_ADJUSTMENT = 20;
  const HOME_FIELD_LOG_ODDS = 0.1603;

  function shiftLogOdds(p: number, delta: number): number {
    const clamped = Math.max(1e-9, Math.min(1 - 1e-9, p));
    const lo = Math.log(clamped / (1 - clamped)) + delta;
    return 1 / (1 + Math.exp(-lo));
  }

  function effectiveRA(rs: number, ra: number, g: number, pERA: number | null, pIP: number | null): number {
    const teamRAG = g > 0 ? ra / g : 4.5;
    if (!applyPitchers) return teamRAG;
    if (pERA !== null && pIP !== null && pIP >= MIN_IP_FOR_ADJUSTMENT) {
      return STARTER_SHARE * pERA + (1 - STARTER_SHARE) * teamRAG;
    }
    return teamRAG;
  }

  // Sort state for left team table
  type SortKey = "rank" | "team" | "wpct" | "rpg" | "rapg" | "os" | "ds";
  let sortKey = $state<SortKey>("rank");
  let sortDir = $state<"asc" | "desc">("asc");

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

  let result = $derived.by(() => {
    // Per-game rates. RS/G is unchanged by pitcher (offense), but RA/G blends with the starter.
    const homeRSG = homeG > 0 ? homeRS / homeG : 4.5;
    const awayRSG = awayG > 0 ? awayRS / awayG : 4.5;
    const homeRAEff = effectiveRA(homeRS, homeRA, homeG, homePitcherERA, homePitcherIP);
    const awayRAEff = effectiveRA(awayRS, awayRA, awayG, awayPitcherERA, awayPitcherIP);

    // Pythag uses per-game rates; the exponent is the same as game-totals form (scale-invariant).
    const pHome = pythag(homeRSG, homeRAEff, exponent);
    const pAway = pythag(awayRSG, awayRAEff, exponent);
    const neutralHomeWin = log5(pHome, pAway);
    const homeWin = applyHomeField
      ? shiftLogOdds(neutralHomeWin, HOME_FIELD_LOG_ODDS)
      : neutralHomeWin;
    const awayWin = 1 - homeWin;

    const osH = homeRSG / leagueAvgRuns;
    const dsH = homeRAEff / leagueAvgRuns;
    const osA = awayRSG / leagueAvgRuns;
    const dsA = awayRAEff / leagueAvgRuns;
    const eHome = osH * dsA * leagueAvgRuns;
    const eAway = osA * dsH * leagueAvgRuns;
    return {
      pHome, pAway, homeWin, awayWin,
      homeFairOdds: americanOdds(homeWin),
      awayFairOdds: americanOdds(awayWin),
      eHome, eAway, total: eHome + eAway,
      osH, dsH, osA, dsA,
      homeRAEff, awayRAEff,
    };
  });

  let curve = $derived.by(() => {
    const points: { x: number; y: number }[] = [];
    const steps = 80;
    const min = 0.5;
    const max = 4.0;
    const homeRSG = homeG > 0 ? homeRS / homeG : 4.5;
    const awayRSG = awayG > 0 ? awayRS / awayG : 4.5;
    const homeRAEff = effectiveRA(homeRS, homeRA, homeG, homePitcherERA, homePitcherIP);
    const awayRAEff = effectiveRA(awayRS, awayRA, awayG, awayPitcherERA, awayPitcherIP);
    for (let i = 0; i < steps; i++) {
      const e = min + ((max - min) * i) / (steps - 1);
      const ph = pythag(homeRSG, homeRAEff, e);
      const pa = pythag(awayRSG, awayRAEff, e);
      const neutral = log5(ph, pa);
      const y = applyHomeField ? shiftLogOdds(neutral, HOME_FIELD_LOG_ODDS) : neutral;
      points.push({ x: e, y });
    }
    return { points, min, max };
  });

  // Sorted team table for the left list
  type Row = {
    team_id: number;
    team: string;
    rank: number;
    wpct: number;
    rpg: number;
    rapg: number;
    os: number;
    ds: number;
    runs_scored: number;
    runs_allowed: number;
    games_played: number;
  };

  let rows = $derived.by<Row[]>(() => {
    if (teams.length === 0) return [];
    const ranked = [...teams].sort((a, b) => b.pythag_win_pct - a.pythag_win_pct);
    const rankMap = new Map<number, number>();
    ranked.forEach((t, i) => rankMap.set(t.team_id, i + 1));
    return teams.map((t) => ({
      team_id: t.team_id,
      team: t.team,
      rank: rankMap.get(t.team_id) ?? 0,
      wpct: t.pythag_win_pct,
      rpg: t.games_played > 0 ? t.runs_scored / t.games_played : 0,
      rapg: t.games_played > 0 ? t.runs_allowed / t.games_played : 0,
      os: t.os,
      ds: t.ds,
      runs_scored: t.runs_scored,
      runs_allowed: t.runs_allowed,
      games_played: t.games_played,
    }));
  });

  let sortedRows = $derived.by<Row[]>(() => {
    const arr = [...rows];
    arr.sort((a, b) => {
      let av: number | string = a[sortKey];
      let bv: number | string = b[sortKey];
      if (typeof av === "string" && typeof bv === "string") {
        return sortDir === "asc" ? av.localeCompare(bv) : bv.localeCompare(av);
      }
      return sortDir === "asc"
        ? (av as number) - (bv as number)
        : (bv as number) - (av as number);
    });
    return arr;
  });

  function setSort(k: SortKey) {
    if (sortKey === k) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = k;
      // Sensible default direction per column
      sortDir = k === "team" || k === "rank" ? "asc" : "desc";
    }
  }

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
        assignSide("home", teams[0]);
        assignSide("away", teams[1]);
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function assignSide(side: "home" | "away", t: TeamStats) {
    if (side === "home") {
      homeId = t.team_id;
      homeRS = t.runs_scored;
      homeRA = t.runs_allowed;
      homeG = t.games_played;
    } else {
      awayId = t.team_id;
      awayRS = t.runs_scored;
      awayRA = t.runs_allowed;
      awayG = t.games_played;
    }
  }

  function pickHome(t: Row) {
    const team = teams.find((x) => x.team_id === t.team_id);
    if (!team) return;
    if (awayId === team.team_id) {
      // Swap if the same team is already on the other side
      const prevHome = teams.find((x) => x.team_id === homeId);
      if (prevHome) assignSide("away", prevHome);
    }
    assignSide("home", team);
  }
  function pickAway(t: Row) {
    const team = teams.find((x) => x.team_id === t.team_id);
    if (!team) return;
    if (homeId === team.team_id) {
      const prevAway = teams.find((x) => x.team_id === awayId);
      if (prevAway) assignSide("home", prevAway);
    }
    assignSide("away", team);
  }

  function resetTeam(side: "home" | "away") {
    const id = side === "home" ? homeId : awayId;
    const t = teams.find((x) => x.team_id === id);
    if (t) assignSide(side, t);
  }

  function resetExponent() {
    exponent = optimalExponent || 1.83;
  }

  function homeName(): string {
    return teams.find((t) => t.team_id === homeId)?.team ?? "—";
  }
  function awayName(): string {
    return teams.find((t) => t.team_id === awayId)?.team ?? "—";
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

  function sortArrow(k: SortKey): string {
    if (sortKey !== k) return "";
    return sortDir === "asc" ? " ↑" : " ↓";
  }
</script>

<section>
  <header class="hero">
    <h1>Playground</h1>
    <p class="subtle">
      Pick a home and away team from the list. Tweak their runs or the Pythagorean exponent,
      and watch the matchup math update in real time.
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
    <div class="layout">

      <!-- ─── LEFT: TEAM LIST ─── -->
      <aside class="card team-list">
        <header class="list-hdr">
          <h2>Teams</h2>
          <span class="subtle small">Click <span class="badge-mini">H</span> or <span class="badge-mini">A</span> to assign a side</span>
        </header>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th class="num sortable" onclick={() => setSort("rank")}>#{sortArrow("rank")}</th>
                <th class="team-col sortable" onclick={() => setSort("team")}>Team{sortArrow("team")}</th>
                <th class="num sortable" onclick={() => setSort("wpct")}>W%{sortArrow("wpct")}</th>
                <th class="num sortable" onclick={() => setSort("rpg")}>R/G{sortArrow("rpg")}</th>
                <th class="num sortable" onclick={() => setSort("rapg")}>RA/G{sortArrow("rapg")}</th>
                <th class="num sortable" onclick={() => setSort("os")}>OS{sortArrow("os")}</th>
                <th class="num sortable" onclick={() => setSort("ds")}>DS{sortArrow("ds")}</th>
                <th class="pick">Pick</th>
              </tr>
            </thead>
            <tbody>
              {#each sortedRows as r (r.team_id)}
                {@const isHome = r.team_id === homeId}
                {@const isAway = r.team_id === awayId}
                <tr class:rowhome={isHome} class:rowaway={isAway}>
                  <td class="num dim">{r.rank}</td>
                  <td class="team-col">{r.team}</td>
                  <td class="num mono">{fmtPct(r.wpct, 1)}</td>
                  <td class="num mono">{r.rpg.toFixed(2)}</td>
                  <td class="num mono">{r.rapg.toFixed(2)}</td>
                  <td class="num mono">{r.os.toFixed(2)}</td>
                  <td class="num mono">{r.ds.toFixed(2)}</td>
                  <td class="pick">
                    <button
                      class="pill"
                      class:active={isHome}
                      title="Set as Home"
                      onclick={() => pickHome(r)}
                    >H</button>
                    <button
                      class="pill"
                      class:active={isAway}
                      title="Set as Away"
                      onclick={() => pickAway(r)}
                    >A</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </aside>

      <!-- ─── RIGHT: MATCHUP EDITOR ─── -->
      <div class="editor">

        <!-- Matchup header (selected teams + edit fields) -->
        <div class="card matchup-card">
          <div class="matchup-head">
            <div class="sel away-sel">
              <span class="role">Away</span>
              <h3>{awayName()}</h3>
            </div>
            <div class="vs">vs</div>
            <div class="sel home-sel">
              <span class="role">Home</span>
              <h3>{homeName()}</h3>
            </div>
          </div>

          <div class="apply-row">
            <label class="apply-toggle">
              <span class="lbl">
                Apply Pitcher Adjustment
                <InfoTip text="Off = pure Pythagorean from RS/RA. On = blend 60% starter ERA + 40% team RA/G for each side (when ERA + IP are set and IP ≥ 20)." />
              </span>
              <button
                class="toggle"
                class:on={applyPitchers}
                role="switch"
                aria-checked={applyPitchers}
                onclick={() => applyPitchers = !applyPitchers}
              >
                <span class="thumb"></span>
                <span class="track-label on-label">On</span>
                <span class="track-label off-label">Off</span>
              </button>
            </label>
            <label class="apply-toggle">
              <span class="lbl">
                Apply Home Field
                <InfoTip text="Adds a log-odds shift to home win probability matching MLB's ~54% historical home win rate. Shrinks at the extremes." />
              </span>
              <button
                class="toggle"
                class:on={applyHomeField}
                role="switch"
                aria-checked={applyHomeField}
                onclick={() => applyHomeField = !applyHomeField}
              >
                <span class="thumb"></span>
                <span class="track-label on-label">On</span>
                <span class="track-label off-label">Off</span>
              </button>
            </label>
          </div>

          <div class="sides">
            <!-- AWAY editable -->
            <div class="side">
              <div class="side-hdr">
                <span class="lbl">Away · {awayName()}</span>
                <button class="ghost small" onclick={() => resetTeam("away")}>Reset</button>
              </div>
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
              <div class="pitcher-row">
                <span class="lbl">
                  Starter
                  <InfoTip text="Optional. If set, the team's effective RA blends 60% starter ERA + 40% team RA/G. Needs IP ≥ 20 to apply." />
                </span>
                <div class="pitcher-fields">
                  <label>
                    <span>ERA</span>
                    <input type="number" min="0" step="0.01" placeholder="—"
                      bind:value={awayPitcherERA} />
                  </label>
                  <label>
                    <span>IP</span>
                    <input type="number" min="0" step="0.1" placeholder="—"
                      bind:value={awayPitcherIP} />
                  </label>
                  <button class="ghost small" onclick={() => { awayPitcherERA = null; awayPitcherIP = null; }}>
                    Clear
                  </button>
                </div>
              </div>
              <p class="readouts mono">
                Pythag W%: <strong>{fmtPct(result.pAway, 1)}</strong>
                &nbsp;·&nbsp; OS: {result.osA.toFixed(2)}
                &nbsp;·&nbsp; DS: {result.dsA.toFixed(2)}
                &nbsp;·&nbsp; eff RA: {result.awayRAEff.toFixed(2)}
              </p>
            </div>

            <!-- HOME editable -->
            <div class="side">
              <div class="side-hdr">
                <span class="lbl">Home · {homeName()}</span>
                <button class="ghost small" onclick={() => resetTeam("home")}>Reset</button>
              </div>
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
              <div class="pitcher-row">
                <span class="lbl">
                  Starter
                  <InfoTip text="Optional. If set, the team's effective RA blends 60% starter ERA + 40% team RA/G. Needs IP ≥ 20 to apply." />
                </span>
                <div class="pitcher-fields">
                  <label>
                    <span>ERA</span>
                    <input type="number" min="0" step="0.01" placeholder="—"
                      bind:value={homePitcherERA} />
                  </label>
                  <label>
                    <span>IP</span>
                    <input type="number" min="0" step="0.1" placeholder="—"
                      bind:value={homePitcherIP} />
                  </label>
                  <button class="ghost small" onclick={() => { homePitcherERA = null; homePitcherIP = null; }}>
                    Clear
                  </button>
                </div>
              </div>
              <p class="readouts mono">
                Pythag W%: <strong>{fmtPct(result.pHome, 1)}</strong>
                &nbsp;·&nbsp; OS: {result.osH.toFixed(2)}
                &nbsp;·&nbsp; DS: {result.dsH.toFixed(2)}
                &nbsp;·&nbsp; eff RA: {result.homeRAEff.toFixed(2)}
              </p>
            </div>
          </div>
        </div>

        <!-- Exponent slider -->
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

        <!-- Live results -->
        <div class="card result-card">
          <div class="result-grid">
            <div class="result-block">
              <span class="lbl">Away Win %</span>
              <div class="big" class:fav={result.awayWin > 0.5}>{fmtPct(result.awayWin, 1)}</div>
              <span class="mono">{fmtOdds(result.awayFairOdds)} fair</span>
            </div>
            <div class="result-block">
              <span class="lbl">Home Win %</span>
              <div class="big" class:fav={result.homeWin >= 0.5}>{fmtPct(result.homeWin, 1)}</div>
              <span class="mono">{fmtOdds(result.homeFairOdds)} fair</span>
            </div>
            <div class="result-block">
              <span class="lbl">League Avg Runs</span>
              <input type="number" step="0.05" min="3" max="6" bind:value={leagueAvgRuns} class="lgavg" />
              <span class="subtle small">per team / game</span>
            </div>
            <div class="result-block">
              <span class="lbl">Predicted Away Runs</span>
              <div class="big">{fmtRuns(result.eAway)}</div>
            </div>
            <div class="result-block">
              <span class="lbl">Predicted Home Runs</span>
              <div class="big">{fmtRuns(result.eHome)}</div>
            </div>
            <div class="result-block">
              <span class="lbl">Total Runs</span>
              <div class="big">{fmtRuns(result.total)}</div>
            </div>
          </div>
        </div>

        <!-- Sensitivity chart -->
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
            <line
              x1={PX}
              x2={CW - PX}
              y1={CH - PY - (CH - PY * 2) * 0.5}
              y2={CH - PY - (CH - PY * 2) * 0.5}
              class="axis-mid"
            />
            <path d={pathD} class="curve" />
            <line x1={markerX} x2={markerX} y1={PY} y2={CH - PY} class="marker-x" />
            <circle cx={markerX} cy={markerY} r="4" class="marker-pt" />
            <text x="4" y={PY + 6} class="tick">100%</text>
            <text x="4" y={CH - PY + 4} class="tick">0%</text>
            <text x="4" y={CH - PY - (CH - PY * 2) * 0.5 + 4} class="tick">50%</text>
            <text x={PX} y={CH - 1} class="tick">0.5</text>
            <text x={PX + (CW - PX * 2) * (2.0 - 0.5) / (4 - 0.5)} y={CH - 1} class="tick" text-anchor="middle">2.0</text>
            <text x={CW - PX} y={CH - 1} class="tick" text-anchor="end">4.0</text>
          </svg>
        </div>

      </div>
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

  /* ─── TWO-PANE LAYOUT ─── */
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
    gap: 16px;
    align-items: stretch;
  }
  @media (max-width: 1000px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  /* LEFT: team list */
  .team-list {
    padding: 0;
    overflow: hidden;
    min-width: 0;
  }
  .list-hdr {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--line-soft);
    gap: 10px;
  }
  .list-hdr h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .table-wrap {
    overflow-x: auto;
  }
  .team-list table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .team-list th {
    position: sticky;
    top: 0;
    background: var(--bg-elev);
    text-align: left;
    padding: 8px 10px;
    font-weight: 500;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
    border-bottom: 1px solid var(--line-soft);
    user-select: none;
  }
  .team-list th.sortable {
    cursor: pointer;
  }
  .team-list th.sortable:hover {
    color: var(--ink);
  }
  .team-list th.num,
  .team-list td.num {
    text-align: right;
  }
  .team-list td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--line-soft);
    line-height: 1.2;
  }
  .team-list tr:last-child td {
    border-bottom: none;
  }
  .team-list tr:hover td {
    background: var(--bg-soft);
  }
  .team-col {
    white-space: nowrap;
    color: var(--ink);
  }
  .dim {
    color: var(--ink-mute);
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .pick {
    text-align: center;
    white-space: nowrap;
  }
  .pill {
    display: inline-block;
    min-width: 26px;
    padding: 3px 7px;
    margin: 0 1px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--ink-soft);
    font-family: var(--mono);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    line-height: 1;
  }
  .pill:hover {
    border-color: var(--ink);
    color: var(--ink);
  }
  .pill.active {
    background: var(--good);
    border-color: var(--good);
    color: var(--bg);
  }
  .rowhome td {
    background: color-mix(in srgb, var(--good) 8%, transparent);
  }
  .rowaway td {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .rowhome:hover td {
    background: color-mix(in srgb, var(--good) 14%, transparent);
  }
  .rowaway:hover td {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .badge-mini {
    display: inline-block;
    padding: 0 5px;
    border: 1px solid var(--line);
    border-radius: 3px;
    font-family: var(--mono);
    font-size: 0.7rem;
    color: var(--ink-soft);
  }

  /* RIGHT: matchup editor */
  .editor {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .matchup-card {
    padding: 16px 18px;
  }
  .matchup-head {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 14px;
    align-items: center;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line-soft);
    margin-bottom: 14px;
  }
  .sel { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .sel.away-sel { text-align: left; }
  .sel.home-sel { text-align: right; }
  .sel h3 {
    margin: 0;
    font-family: var(--serif);
    font-size: 1.4rem;
    line-height: 1.1;
    color: var(--ink);
    overflow-wrap: anywhere;
  }
  .role {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--ink-mute);
  }
  .vs {
    font-family: var(--mono);
    color: var(--ink-mute);
    font-size: 0.9rem;
  }
  .sides {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  @media (max-width: 700px) {
    .sides { grid-template-columns: 1fr; }
  }
  .side {
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    min-width: 0;
  }
  .side-hdr {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    gap: 8px;
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
    gap: 8px;
    margin-bottom: 8px;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.74rem;
    color: var(--ink-soft);
    min-width: 0;
  }
  .fields input {
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }
  .readouts {
    color: var(--ink-soft);
    font-size: 0.8rem;
    margin: 4px 0 0;
  }
  .apply-row {
    display: flex;
    justify-content: flex-end;
    gap: 24px;
    flex-wrap: wrap;
    margin-bottom: 14px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line-soft);
  }
  .apply-toggle {
    display: flex;
    align-items: center;
    gap: 12px;
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
    transition: opacity 0.18s ease;
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

  .pitcher-row {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px dashed var(--line-soft);
  }
  .pitcher-fields {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 8px;
    align-items: end;
    margin-top: 4px;
  }
  .pitcher-fields label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.72rem;
    color: var(--ink-soft);
    min-width: 0;
  }
  .pitcher-fields input {
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }
  .small {
    font-size: 0.82rem;
  }

  .sliderhdr {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    margin-bottom: 8px;
    gap: 10px;
  }
  .sliderhdr h2 {
    font-family: var(--mono);
    font-size: 1.8rem;
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

  .result-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
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
    min-width: 0;
  }
  .result-block .big {
    font-family: var(--serif);
    font-size: 1.55rem;
    font-weight: 600;
  }
  .result-block .big.fav {
    color: var(--good);
  }
  .lgavg {
    max-width: 90px;
    margin-top: 2px;
  }

  .chart-card {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .chart {
    width: 100%;
    height: 170px;
    flex: 1;
    min-height: 170px;
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
