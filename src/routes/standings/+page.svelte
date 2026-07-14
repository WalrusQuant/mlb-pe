<script lang="ts">
  import { onMount } from "svelte";
  import { getStandings } from "$lib/api";
  import type { StandingsBundle, TeamStanding } from "$lib/types";
  import { relativeTime } from "$lib/format";

  let loading = $state(true);
  let error = $state<string | null>(null);
  let bundle = $state<StandingsBundle | null>(null);

  // Division ID → human-readable name. From MLB's API.
  const DIVISION_NAMES: Record<number, string> = {
    200: "AL West",
    201: "AL East",
    202: "AL Central",
    203: "NL West",
    204: "NL East",
    205: "NL Central",
  };
  // The grid fills row-by-row: keep AL in the left column and NL in the right,
  // with matching East / Central / West divisions on each row.
  const DIVISION_ORDER: number[] = [201, 204, 202, 205, 200, 203];

  // League ID → display name.
  const LEAGUE_NAMES: Record<number, string> = {
    103: "American League",
    104: "National League",
  };

  async function load() {
    loading = true;
    error = null;
    try {
      bundle = await getStandings();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Group teams by division for the division tables.
  let byDivision = $derived.by<Record<number, TeamStanding[]>>(() => {
    if (!bundle) return {};
    const groups: Record<number, TeamStanding[]> = {};
    for (const t of bundle.teams) {
      (groups[t.divisionId] ??= []).push(t);
    }
    // Each division: sort by division rank.
    for (const div in groups) {
      groups[div].sort((a, b) => (a.divisionRank ?? 99) - (b.divisionRank ?? 99));
    }
    return groups;
  });

  // Wild card race: per league, all non-division-leaders ranked by wildCardRank.
  let wildCardByLeague = $derived.by<Record<number, TeamStanding[]>>(() => {
    if (!bundle) return {};
    const groups: Record<number, TeamStanding[]> = {};
    for (const t of bundle.teams) {
      if (t.divisionLeader) continue;
      if (t.wildCardRank == null) continue;
      (groups[t.leagueId] ??= []).push(t);
    }
    for (const lg in groups) {
      groups[lg].sort((a, b) => (a.wildCardRank ?? 99) - (b.wildCardRank ?? 99));
    }
    return groups;
  });

  function streakClass(code: string | null): string {
    if (!code) return "";
    if (code.startsWith("W")) return "streak-win";
    if (code.startsWith("L")) return "streak-loss";
    return "";
  }

  function diffClass(diff: number): string {
    if (diff > 0) return "diff-pos";
    if (diff < 0) return "diff-neg";
    return "";
  }

  function fmtDiff(diff: number): string {
    if (diff > 0) return `+${diff}`;
    return String(diff);
  }

  onMount(load);
</script>

<section>
  <header class="hero">
    <div>
      <h1>Standings</h1>
      <p class="subtle">
        Live MLB standings — divisions on top, wild-card race per league below.
        Data direct from <span class="mono">statsapi.mlb.com</span>.
      </p>
    </div>
  </header>

  {#if error}
    <div class="card err">
      <strong>Couldn't load standings.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading && !bundle}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Loading standings…</p>
    </div>
  {:else if bundle}
    <div class="meta">
      <span class="badge">Season {bundle.season}</span>
      <span class="badge">Updated {relativeTime(bundle.lastUpdated)}</span>
    </div>

    <!-- Division tables — 2x3 grid -->
    <div class="div-grid">
      {#each DIVISION_ORDER as divId}
        {@const teams = byDivision[divId] ?? []}
        {#if teams.length > 0}
          <div class="card div-card">
            <header class="div-hdr">
              <h2>{DIVISION_NAMES[divId]}</h2>
            </header>
            <table>
              <thead>
                <tr>
                  <th class="team-col">Team</th>
                  <th class="num">W</th>
                  <th class="num">L</th>
                  <th class="num">PCT</th>
                  <th class="num">GB</th>
                  <th class="num">L10</th>
                  <th class="num">Strk</th>
                  <th class="num">Diff</th>
                </tr>
              </thead>
              <tbody>
                {#each teams as t (t.teamId)}
                  <tr>
                    <td class="team-col">
                      {#if t.divisionLeader}<span class="crown" title="Division leader">★</span>{/if}
                      <span class="tname">{t.teamName}</span>
                    </td>
                    <td class="num mono">{t.wins}</td>
                    <td class="num mono">{t.losses}</td>
                    <td class="num mono">{t.pct}</td>
                    <td class="num mono">{t.gamesBack}</td>
                    <td class="num mono">{t.lastTenWins}-{t.lastTenLosses}</td>
                    <td class="num mono {streakClass(t.streakCode)}">{t.streakCode ?? "-"}</td>
                    <td class="num mono {diffClass(t.runDifferential)}">{fmtDiff(t.runDifferential)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/each}
    </div>

    <!-- Wild card race per league -->
    <div class="wc-grid">
      {#each [103, 104] as lgId}
        {@const teams = wildCardByLeague[lgId] ?? []}
        {#if teams.length > 0}
          <div class="card wc-card">
            <header class="wc-hdr">
              <h2>{LEAGUE_NAMES[lgId]} Wild Card</h2>
              <span class="subtle small">Top 3 advance · everyone below is chasing</span>
            </header>
            <table>
              <thead>
                <tr>
                  <th class="num">WC</th>
                  <th class="team-col">Team</th>
                  <th class="num">W</th>
                  <th class="num">L</th>
                  <th class="num">PCT</th>
                  <th class="num">WC GB</th>
                  <th class="num">L10</th>
                  <th class="num">Strk</th>
                </tr>
              </thead>
              <tbody>
                {#each teams as t (t.teamId)}
                  <tr class:in-line={t.wildCardRank != null && t.wildCardRank <= 3}>
                    <td class="num mono">{t.wildCardRank}</td>
                    <td class="team-col">
                      <span class="tname">{t.teamName}</span>
                    </td>
                    <td class="num mono">{t.wins}</td>
                    <td class="num mono">{t.losses}</td>
                    <td class="num mono">{t.pct}</td>
                    <td class="num mono">{t.wildCardGamesBack}</td>
                    <td class="num mono">{t.lastTenWins}-{t.lastTenLosses}</td>
                    <td class="num mono {streakClass(t.streakCode)}">{t.streakCode ?? "-"}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/each}
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
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 14px 0;
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
    to {
      transform: rotate(360deg);
    }
  }

  /* Division grid: 2 columns on wide, 1 on narrow */
  .div-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 20px;
  }
  @media (max-width: 900px) {
    .div-grid {
      grid-template-columns: 1fr;
    }
  }
  .div-card {
    padding: 14px 16px;
  }
  .div-hdr h2 {
    margin: 0 0 10px;
    font-size: 1.1rem;
  }

  /* Wild card grid: 2 columns */
  .wc-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  @media (max-width: 900px) {
    .wc-grid {
      grid-template-columns: 1fr;
    }
  }
  .wc-card {
    padding: 14px 16px;
  }
  .wc-hdr {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 10px;
    gap: 10px;
  }
  .wc-hdr h2 {
    margin: 0;
    font-size: 1.05rem;
  }

  /* Tables share styling */
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
  th.num,
  td.num {
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
  .tname {
    font-weight: 500;
  }
  .crown {
    color: var(--good);
    margin-right: 4px;
    font-size: 0.9em;
  }
  .mono {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .streak-win { color: var(--good); }
  .streak-loss { color: var(--bad); }
  .diff-pos { color: var(--good); }
  .diff-neg { color: var(--bad); }

  /* Subtle highlight for teams currently in WC position */
  .wc-card tr.in-line td {
    background: color-mix(in srgb, var(--good) 6%, transparent);
  }
  .wc-card tr.in-line:hover td {
    background: color-mix(in srgb, var(--good) 12%, transparent);
  }
</style>
