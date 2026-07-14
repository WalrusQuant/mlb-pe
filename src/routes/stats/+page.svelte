<script lang="ts">
  import { onMount } from "svelte";
  import { getStandings, getTeamStats } from "$lib/api";
  import type { TeamStats, TeamStanding } from "$lib/types";
  import { fmtPct, relativeTime } from "$lib/format";

  let loading = $state(true);
  let error = $state<string | null>(null);
  let teams = $state<TeamStats[]>([]);
  let standings = $state<TeamStanding[]>([]);
  let lastUpdated = $state<string>("");
  let exponent = $state<number>(0);

  // Mirrors backend MIN_RECENT_GAMES. Below this we skip a team from the hot/cold
  // table — small samples make the L20 delta meaningless.
  const MIN_RECENT_GAMES = 10;

  async function load() {
    loading = true;
    error = null;
    try {
      const [ts, st] = await Promise.all([getTeamStats(), getStandings()]);
      teams = ts.teams;
      exponent = ts.exponent;
      standings = st.teams;
      lastUpdated = st.lastUpdated;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  type Joined = {
    team_id: number;
    team: string;
    games_played: number;
    actual_pct: number;       // 0..1
    pythag_pct: number;       // 0..1
    luck: number;             // actual - pythag (positive = over-performing)
    rpg: number;
    rapg: number;
    os: number;
    ds: number;
    net_season: number;       // RS/G - RA/G
    recent_games: number | null;
    recent_rs_per_game: number | null;
    recent_ra_per_game: number | null;
    net_recent: number | null;  // recent RS/G - recent RA/G
    swing: number | null;       // net_recent - net_season
  };

  let joined = $derived.by<Joined[]>(() => {
    if (teams.length === 0 || standings.length === 0) return [];
    const stById = new Map<number, TeamStanding>();
    for (const s of standings) stById.set(s.teamId, s);
    const out: Joined[] = [];
    for (const t of teams) {
      const st = stById.get(t.teamId);
      if (!st) continue;
      const gp = t.gamesPlayed;
      if (gp === 0) continue;
      const rpg = t.runsScored / gp;
      const rapg = t.runsAllowed / gp;
      const net_season = rpg - rapg;
      const actual_pct = st.wins + st.losses > 0 ? st.wins / (st.wins + st.losses) : 0;
      const recent_games = t.recentGames;
      const recent_rs = t.recentRsPerGame;
      const recent_ra = t.recentRaPerGame;
      const net_recent = recent_rs != null && recent_ra != null ? recent_rs - recent_ra : null;
      const swing = net_recent != null ? net_recent - net_season : null;
      out.push({
        team_id: t.teamId,
        team: t.team,
        games_played: gp,
        actual_pct,
        pythag_pct: t.pythagWinPct,
        luck: actual_pct - t.pythagWinPct,
        rpg,
        rapg,
        os: t.os,
        ds: t.ds,
        net_season,
        recent_games,
        recent_rs_per_game: recent_rs,
        recent_ra_per_game: recent_ra,
        net_recent,
        swing,
      });
    }
    return out;
  });

  let luckOver = $derived.by(() =>
    [...joined].sort((a, b) => b.luck - a.luck).slice(0, 5),
  );
  let luckUnder = $derived.by(() =>
    [...joined].sort((a, b) => a.luck - b.luck).slice(0, 5),
  );
  let osTop = $derived.by(() =>
    [...joined].sort((a, b) => b.os - a.os).slice(0, 5),
  );
  let dsTop = $derived.by(() =>
    [...joined].sort((a, b) => a.ds - b.ds).slice(0, 5),  // lower DS = better defense
  );
  let hotTop = $derived.by(() =>
    joined
      .filter((j) => j.swing !== null && (j.recent_games ?? 0) >= MIN_RECENT_GAMES)
      .sort((a, b) => (b.swing ?? 0) - (a.swing ?? 0))
      .slice(0, 5),
  );
  let coldTop = $derived.by(() =>
    joined
      .filter((j) => j.swing !== null && (j.recent_games ?? 0) >= MIN_RECENT_GAMES)
      .sort((a, b) => (a.swing ?? 0) - (b.swing ?? 0))
      .slice(0, 5),
  );

  function fmtDelta(d: number, places = 3): string {
    const s = d.toFixed(places);
    return d > 0 ? `+${s}` : s;
  }
  function fmtPctDelta(d: number): string {
    const pct = d * 100;
    const s = pct.toFixed(1);
    return d > 0 ? `+${s} pts` : `${s} pts`;
  }
  function deltaClass(d: number): string {
    if (d > 0) return "delta-pos";
    if (d < 0) return "delta-neg";
    return "";
  }

  onMount(load);
</script>

<section>
  <header class="hero">
    <h1>Stats</h1>
    <p class="subtle">
      The numbers MLB.com won't show you — every leaderboard here is filtered through
      the Pythagorean lens. Who's lucky, who's running cold, whose offense is doing the heavy
      lifting. Exponent in use: <span class="mono">x = {exponent.toFixed(3)}</span>.
    </p>
  </header>

  {#if error}
    <div class="card err">
      <strong>Couldn't load stats.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Loading team data…</p>
    </div>
  {:else}
    <div class="meta">
      <span class="badge">{joined.length} teams</span>
      {#if lastUpdated}
        <span class="badge">Updated {relativeTime(lastUpdated)}</span>
      {/if}
    </div>

    <!-- ── LUCK ─────────────────────────────────────────────── -->
    <div class="section-hdr">
      <h2>Luck — actual record vs. what the run differential says</h2>
      <p class="subtle small">
        Pythagorean predicts a team's record from how many runs they score and allow.
        Teams above their Pythag are winning more games than their run differential explains
        — usually clutch hitting in close games, sometimes a fluke. Teams below are losing games
        they "should" be winning, often because their wins are blowouts and their losses are tight.
      </p>
    </div>
    <div class="two-up">
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Over-performing <span class="hint">(Actual &gt; Pythag)</span></h3>
        </header>
        <table>
          <thead>
            <tr>
              <th class="team-col">Team</th>
              <th class="num">Actual</th>
              <th class="num">Pythag</th>
              <th class="num">Δ</th>
            </tr>
          </thead>
          <tbody>
            {#each luckOver as t (t.team_id)}
              <tr>
                <td class="team-col"><span class="tname">{t.team}</span></td>
                <td class="num mono">{fmtPct(t.actual_pct, 3)}</td>
                <td class="num mono">{fmtPct(t.pythag_pct, 3)}</td>
                <td class="num mono {deltaClass(t.luck)}">{fmtPctDelta(t.luck)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Under-performing <span class="hint">(Actual &lt; Pythag)</span></h3>
        </header>
        <table>
          <thead>
            <tr>
              <th class="team-col">Team</th>
              <th class="num">Actual</th>
              <th class="num">Pythag</th>
              <th class="num">Δ</th>
            </tr>
          </thead>
          <tbody>
            {#each luckUnder as t (t.team_id)}
              <tr>
                <td class="team-col"><span class="tname">{t.team}</span></td>
                <td class="num mono">{fmtPct(t.actual_pct, 3)}</td>
                <td class="num mono">{fmtPct(t.pythag_pct, 3)}</td>
                <td class="num mono {deltaClass(t.luck)}">{fmtPctDelta(t.luck)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- ── OFFENSE / DEFENSE ────────────────────────────────── -->
    <div class="section-hdr">
      <h2>Offense and Defense, indexed to the league</h2>
      <p class="subtle small">
        OS = (R/G) ÷ league avg. DS = (RA/G) ÷ league avg. An OS of 1.20 means the team scores
        20% more than league average. For DS, lower is better — a DS of 0.85 means a team allows
        15% fewer runs than league average.
      </p>
    </div>
    <div class="two-up">
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Best offenses <span class="hint">(highest OS)</span></h3>
        </header>
        <table>
          <thead>
            <tr>
              <th class="team-col">Team</th>
              <th class="num">R/G</th>
              <th class="num">OS</th>
            </tr>
          </thead>
          <tbody>
            {#each osTop as t (t.team_id)}
              <tr>
                <td class="team-col"><span class="tname">{t.team}</span></td>
                <td class="num mono">{t.rpg.toFixed(2)}</td>
                <td class="num mono good-strong">{t.os.toFixed(2)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Best defenses <span class="hint">(lowest DS)</span></h3>
        </header>
        <table>
          <thead>
            <tr>
              <th class="team-col">Team</th>
              <th class="num">RA/G</th>
              <th class="num">DS</th>
            </tr>
          </thead>
          <tbody>
            {#each dsTop as t (t.team_id)}
              <tr>
                <td class="team-col"><span class="tname">{t.team}</span></td>
                <td class="num mono">{t.rapg.toFixed(2)}</td>
                <td class="num mono good-strong">{t.ds.toFixed(2)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- ── HOT / COLD ───────────────────────────────────────── -->
    <div class="section-hdr">
      <h2>Hot and Cold — L20 net runs vs. season net runs</h2>
      <p class="subtle small">
        Net = (RS/G − RA/G). Teams are sorted by how much their last-20-games net differs from
        their season-long net. A big positive swing is a team playing better than they have all
        year; a big negative is a team in a real slump. Teams with fewer than {MIN_RECENT_GAMES} completed
        games are omitted.
      </p>
    </div>
    <div class="two-up">
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Hottest <span class="hint">(L20 net &gt; season)</span></h3>
        </header>
        {#if hotTop.length === 0}
          <p class="empty-line subtle small">Not enough completed games this season yet.</p>
        {:else}
          <table>
            <thead>
              <tr>
                <th class="team-col">Team</th>
                <th class="num">Season</th>
                <th class="num">L{Math.min(20, hotTop[0].recent_games ?? 20)}</th>
                <th class="num">Swing</th>
              </tr>
            </thead>
            <tbody>
              {#each hotTop as t (t.team_id)}
                <tr>
                  <td class="team-col"><span class="tname">{t.team}</span></td>
                  <td class="num mono">{fmtDelta(t.net_season, 2)}</td>
                  <td class="num mono">{fmtDelta(t.net_recent ?? 0, 2)}</td>
                  <td class="num mono {deltaClass(t.swing ?? 0)}">{fmtDelta(t.swing ?? 0, 2)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
      <div class="card stat-card">
        <header class="stat-hdr">
          <h3>Coldest <span class="hint">(L20 net &lt; season)</span></h3>
        </header>
        {#if coldTop.length === 0}
          <p class="empty-line subtle small">Not enough completed games this season yet.</p>
        {:else}
          <table>
            <thead>
              <tr>
                <th class="team-col">Team</th>
                <th class="num">Season</th>
                <th class="num">L{Math.min(20, coldTop[0].recent_games ?? 20)}</th>
                <th class="num">Swing</th>
              </tr>
            </thead>
            <tbody>
              {#each coldTop as t (t.team_id)}
                <tr>
                  <td class="team-col"><span class="tname">{t.team}</span></td>
                  <td class="num mono">{fmtDelta(t.net_season, 2)}</td>
                  <td class="num mono">{fmtDelta(t.net_recent ?? 0, 2)}</td>
                  <td class="num mono {deltaClass(t.swing ?? 0)}">{fmtDelta(t.swing ?? 0, 2)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .hero {
    margin-bottom: 22px;
  }
  .hero p {
    max-width: 70ch;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 14px 0 18px;
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
    to { transform: rotate(360deg); }
  }

  .section-hdr {
    margin: 28px 0 10px;
  }
  .section-hdr h2 {
    margin: 0 0 4px;
    font-family: var(--serif);
    font-size: 1.25rem;
    color: var(--ink);
  }
  .section-hdr p {
    max-width: 78ch;
    margin: 0;
  }

  .two-up {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  @media (max-width: 900px) {
    .two-up { grid-template-columns: 1fr; }
  }

  .stat-card {
    padding: 14px 16px;
  }
  .stat-hdr h3 {
    margin: 0 0 10px;
    font-size: 1rem;
    color: var(--ink);
  }
  .hint {
    font-size: 0.75rem;
    color: var(--ink-mute);
    font-weight: 400;
    margin-left: 4px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }
  th {
    text-align: left;
    padding: 6px 8px;
    font-size: 0.7rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
    border-bottom: 1px solid var(--line-soft);
  }
  th.num, td.num {
    text-align: right;
  }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--line-soft);
    line-height: 1.3;
  }
  tr:last-child td {
    border-bottom: none;
  }
  tr:hover td {
    background: var(--bg-soft);
  }
  .team-col {
    white-space: nowrap;
    color: var(--ink);
  }
  .tname { font-weight: 500; }
  .mono {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .delta-pos { color: var(--good); }
  .delta-neg { color: var(--bad); }
  .good-strong { color: var(--good); font-weight: 600; }
  .empty-line {
    margin: 12px 4px 0;
  }
</style>
