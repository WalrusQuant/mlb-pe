<script lang="ts">
  import { onMount } from "svelte";
  import { getDivisionRace } from "$lib/api";
  import type { DivisionRaceBundle, DivisionRaceRow, WlOverride } from "$lib/types";
  import { fmtPct, relativeTime } from "$lib/format";
  import InfoTip from "$lib/components/InfoTip.svelte";

  const DIVISIONS: { id: number; label: string }[] = [
    { id: 201, label: "AL East" },
    { id: 202, label: "AL Central" },
    { id: 200, label: "AL West" },
    { id: 204, label: "NL East" },
    { id: 205, label: "NL Central" },
    { id: 203, label: "NL West" },
  ];

  let divisionId = $state(201);
  let includeHomeField = $state(true);
  let includeRecentForm = $state(true);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let bundle = $state<DivisionRaceBundle | null>(null);
  let expanded = $state<number | null>(null);
  let edits = $state<Record<number, { wins: number; losses: number }>>({});
  let dirty = $state(false);

  async function load(overrides?: WlOverride[]) {
    loading = true;
    error = null;
    try {
      bundle = await getDivisionRace({
        divisionId,
        includeHomeField,
        includeRecentForm,
        wlOverrides: overrides,
      });
      if (!overrides || overrides.length === 0) {
        const next: Record<number, { wins: number; losses: number }> = {};
        for (const t of bundle.teams) next[t.teamId] = { wins: t.wins, losses: t.losses };
        edits = next;
        dirty = false;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function applyEdits() {
    const overrides: WlOverride[] = Object.entries(edits).map(([id, v]) => ({
      teamId: Number(id),
      wins: Math.max(0, Math.floor(v.wins)),
      losses: Math.max(0, Math.floor(v.losses)),
    }));
    dirty = true;
    load(overrides);
  }

  function resetWl() {
    dirty = false;
    load();
  }

  function onDiv(id: number) {
    divisionId = id;
    expanded = null;
    dirty = false;
    load();
  }

  function toggleRow(id: number) {
    expanded = expanded === id ? null : id;
  }

  function fmtWl(w: number, l: number): string {
    const rw = Math.round(w * 10) / 10;
    const rl = Math.round(l * 10) / 10;
    return `${rw.toFixed(1)}–${rl.toFixed(1)}`;
  }

  function mnLabel(t: DivisionRaceRow): string {
    if (t.clinched) return "Clinched";
    if (t.eliminated) return "Elim.";
    return String(t.magicNumber);
  }

  onMount(() => load());
</script>

<section>
  <header class="hero">
    <div>
      <h1>Division Race</h1>
      <p class="subtle">
        Need vs supply for winning a division. Magic number is the need; rest-of-season
        expected wins (Log5 walk of every remaining opponent) and Monte Carlo
        P(win the division) are the supply. Pitcher blend is off — most remaining
        starters aren't announced.
      </p>
    </div>
  </header>

  <div class="card controls">
    <div class="divs" role="tablist" aria-label="Division">
      {#each DIVISIONS as d (d.id)}
        <button
          type="button"
          class="pill"
          class:on={divisionId === d.id}
          role="tab"
          aria-selected={divisionId === d.id}
          onclick={() => onDiv(d.id)}
        >{d.label}</button>
      {/each}
    </div>
    <label class="tog">
      <span class="lbl">
        Home Field
        <InfoTip text="Log-odds home-field shift on every remaining game (same constant as Predictions). Off = neutral-site Log5." />
      </span>
      <button
        class="toggle"
        class:on={includeHomeField}
        role="switch"
        aria-checked={includeHomeField}
        aria-label="Home field advantage"
        onclick={() => { includeHomeField = !includeHomeField; load(dirty ? Object.entries(edits).map(([id, v]) => ({ teamId: Number(id), wins: v.wins, losses: v.losses })) : undefined); }}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
    <label class="tog">
      <span class="lbl">
        Recent Form
        <InfoTip text="When on, talent uses the same 60/40 season + L20 blend as Predictions. Off = season Pythagorean from RS/RA." />
      </span>
      <button
        class="toggle"
        class:on={includeRecentForm}
        role="switch"
        aria-checked={includeRecentForm}
        aria-label="Recent form"
        onclick={() => { includeRecentForm = !includeRecentForm; load(dirty ? Object.entries(edits).map(([id, v]) => ({ teamId: Number(id), wins: v.wins, losses: v.losses })) : undefined); }}
      >
        <span class="thumb"></span>
        <span class="track-label on-label">On</span>
        <span class="track-label off-label">Off</span>
      </button>
    </label>
  </div>

  {#if error}
    <div class="card err">
      <strong>Couldn't load the division race.</strong>
      <p class="mono small">{error}</p>
    </div>
  {/if}

  {#if loading && !bundle}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Walking the remaining schedule…</p>
    </div>
  {:else if bundle}
    <div class="meta">
      <span class="badge">{bundle.divisionName}</span>
      <span class="badge">Season {bundle.season}</span>
      <span class="badge">G = {bundle.seasonGames}</span>
      <span class="badge">x = {bundle.exponent.toFixed(3)}</span>
      <span class="badge">{bundle.nSims.toLocaleString()} sims</span>
      <span class="badge">Updated {relativeTime(bundle.lastUpdated)}</span>
      {#if dirty}<span class="badge warn">What-if W-L</span>{/if}
    </div>

    <div class="card table-card">
      <table>
        <thead>
          <tr>
            <th class="team-col">Team</th>
            <th class="num" title="Editable — what-if">W</th>
            <th class="num" title="Editable — what-if">L</th>
            <th class="num">RS</th>
            <th class="num">RA</th>
            <th class="num">
              Pythag
              <InfoTip text="Season Pythagorean W% from live RS/RA and the fitted exponent. Talent for the rest-of-season walk; RS/RA are not what-if'd." />
            </th>
            <th class="num">
              MN
              <InfoTip text="Magic number to clinch the division vs the closest threat: G + 1 − W − L of that opponent. A head-to-head win is worth 2. Shrinks by 1 when this team already owns a completed season series." />
            </th>
            <th class="num">Rem</th>
            <th class="num">
              Exp ROS
              <InfoTip text="Expected remaining W-L: each remaining game contributes its Log5 P(win). Out-of-division and interleague opponents count. Pitcher blend off." />
            </th>
            <th class="num">Proj final</th>
            <th class="num">
              P(win)
              <InfoTip text="Share of seeded Monte Carlo season paths where this team uniquely finishes first. Ties that head-to-head cannot break are not counted as a unique win." />
            </th>
          </tr>
        </thead>
        <tbody>
          {#each bundle.teams as t, i (t.teamId)}
            <tr class:open={expanded === t.teamId} class:leader={i === 0 && !t.eliminated}>
              <td class="team-col">
                <button type="button" class="expand" onclick={() => toggleRow(t.teamId)} aria-expanded={expanded === t.teamId}>
                  {#if i === 0 && !t.eliminated}<span class="crown" title="Current leader">★</span>{/if}
                  <span class="tname">{t.teamName}</span>
                  <span class="chev">{expanded === t.teamId ? "▾" : "▸"}</span>
                </button>
              </td>
              <td class="num">
                <input
                  class="wl"
                  type="number"
                  min="0"
                  step="1"
                  bind:value={edits[t.teamId].wins}
                  onchange={applyEdits}
                  aria-label={`${t.teamName} wins`}
                />
              </td>
              <td class="num">
                <input
                  class="wl"
                  type="number"
                  min="0"
                  step="1"
                  bind:value={edits[t.teamId].losses}
                  onchange={applyEdits}
                  aria-label={`${t.teamName} losses`}
                />
              </td>
              <td class="num mono">{t.runsScored}</td>
              <td class="num mono">{t.runsAllowed}</td>
              <td class="num mono">{fmtPct(t.pythagWinPct)}</td>
              <td class="num mono" class:good={t.clinched} class:bad={t.eliminated} title={t.clinched || t.eliminated ? "" : `vs ${t.magicVs}`}>
                {mnLabel(t)}
              </td>
              <td class="num mono">{t.gamesRemaining}</td>
              <td class="num mono">{fmtWl(t.expectedRemainingWins, t.expectedRemainingLosses)}</td>
              <td class="num mono">{fmtWl(t.projectedWins, t.projectedLosses)}</td>
              <td class="num pwin">
                <span class="bar" style={`--w: ${Math.max(0, t.pWinDivision) * 100}%`}></span>
                <span class="mono">{fmtPct(t.pWinDivision, 1)}</span>
              </td>
            </tr>
            {#if expanded === t.teamId}
              <tr class="sched" >
                <td colspan="11">
                  <p class="sched-h">Remaining schedule · Log5 P(win) per game</p>
                  {#if t.remaining.length === 0}
                    <p class="muted small">No remaining games.</p>
                  {:else}
                    <table class="inner">
                      <thead>
                        <tr>
                          <th>Date</th>
                          <th></th>
                          <th>Opponent</th>
                          <th class="num">P(win)</th>
                        </tr>
                      </thead>
                      <tbody>
                        {#each t.remaining as g (g.gamePk)}
                          <tr>
                            <td class="mono">{g.date}</td>
                            <td class="mute">{g.home ? "vs" : "@"}</td>
                            <td>{g.opponent}</td>
                            <td class="num mono">{fmtPct(g.winProb)}</td>
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  {/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
      {#if dirty}
        <div class="resetrow">
          <button type="button" class="ghost" onclick={resetWl}>Reset W-L to live standings</button>
        </div>
      {/if}
    </div>

    {#if bundle.pTie > 0}
      <p class="subtle small">
        P(unresolved tie for first) = {fmtPct(bundle.pTie)} — those paths don't credit anyone with a unique division win.
      </p>
    {/if}
    <p class="subtle small caveat">{bundle.tiebreakerNote}</p>
  {/if}
</section>

<style>
  .hero { margin-bottom: 20px; }
  .hero p { max-width: 72ch; }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 18px 28px;
    align-items: end;
    margin-bottom: 16px;
  }
  .divs { display: flex; flex-wrap: wrap; gap: 6px; }
  .pill {
    background: var(--bg-soft);
    color: var(--ink-soft);
    border: 1px solid var(--line);
    padding: 0.4em 0.8em;
    font-size: 0.88rem;
  }
  .pill.on {
    background: var(--ink);
    color: var(--bg-elev);
    border-color: var(--ink);
  }
  .pill:hover { transform: none; }
  .tog { display: flex; flex-direction: column; gap: 6px; }
  .lbl {
    display: block;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
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
  }
  .toggle:hover { transform: none; }
  .toggle.on { background: var(--good-soft); border-color: var(--good); }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--ink-mute);
    transition: left 0.18s ease, background 0.18s ease;
  }
  .toggle.on .thumb { left: 47px; background: var(--good); }
  .track-label {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    pointer-events: none;
  }
  .on-label { left: 12px; color: var(--good); opacity: 0; }
  .off-label { right: 12px; color: var(--ink-mute); opacity: 1; }
  .toggle.on .on-label { opacity: 1; }
  .toggle.on .off-label { opacity: 0; }

  .meta { display: flex; flex-wrap: wrap; gap: 6px; margin: 14px 0; }
  .badge.warn { color: var(--warn); border-color: var(--warn); }
  .err { border-color: var(--accent); background: var(--accent-soft); color: var(--ink); }
  .center { text-align: center; padding: 40px 20px; }
  .small { font-size: 0.82rem; }
  .spinner {
    display: inline-block;
    width: 18px; height: 18px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: middle;
    margin-right: 8px;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .table-card { padding: 14px 16px; overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
  th {
    text-align: left;
    padding: 6px 8px;
    font-size: 0.7rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
    border-bottom: 1px solid var(--line-soft);
    white-space: nowrap;
  }
  th.num, td.num { text-align: right; }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--line-soft);
    line-height: 1.3;
    vertical-align: middle;
  }
  tr:hover td { background: var(--bg-soft); }
  tr.leader td { background: color-mix(in srgb, var(--good) 6%, transparent); }
  tr.open td { background: var(--bg-soft); }
  .team-col { white-space: nowrap; color: var(--ink); }
  .tname { font-weight: 500; }
  .crown { color: var(--good); margin-right: 4px; }
  .mono { font-family: var(--mono); font-variant-numeric: tabular-nums; }
  .good { color: var(--good); font-weight: 600; }
  .bad { color: var(--ink-mute); }

  .expand {
    background: none;
    border: none;
    color: inherit;
    padding: 0;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .expand:hover { transform: none; background: none; }
  .chev { color: var(--ink-mute); font-size: 0.75rem; }

  .wl {
    width: 4.2em;
    text-align: right;
    font-family: var(--mono);
    padding: 0.2em 0.35em;
    font-size: 0.88rem;
  }

  .pwin {
    position: relative;
    min-width: 7.5em;
  }
  .pwin .bar {
    display: block;
    height: 6px;
    width: 100%;
    background: var(--bg-soft);
    border-radius: 99px;
    margin-bottom: 4px;
    overflow: hidden;
  }
  .pwin .bar::after {
    content: "";
    display: block;
    height: 100%;
    width: var(--w);
    background: var(--good);
  }

  .sched td { background: var(--bg) !important; padding: 10px 12px 16px; }
  .sched-h {
    margin: 0 0 8px;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
  }
  .inner { max-width: 520px; }
  .inner th, .inner td { padding: 4px 8px; font-size: 0.84rem; }
  .mute { color: var(--ink-mute); }
  .resetrow { margin-top: 12px; }
  .caveat { max-width: 90ch; margin-top: 8px; }
</style>
