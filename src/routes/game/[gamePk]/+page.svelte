<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { getGameBreakdown, getGameContext, getStandings } from "$lib/api";
  import type {
    GameBreakdownBundle,
    SideBreakdown,
    GameContextBundle,
    TeamSplits,
    LineupSpot,
    Bullpen,
  } from "$lib/types";
  import { fmtPct, fmtOdds, fmtRuns } from "$lib/format";
  import Formula from "$lib/components/Formula.svelte";
  import InfoTip from "$lib/components/InfoTip.svelte";

  let bundle = $state<GameBreakdownBundle | null>(null);
  let context = $state<GameContextBundle | null>(null);
  let contextLoading = $state(false);
  let recordById = $state<Map<number, string>>(new Map());
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load(gamePk: number, opts: {
    exponent?: number;
    includePitchers: boolean;
    includeHomeField: boolean;
    includeRecentForm: boolean;
  }) {
    loading = true;
    error = null;
    try {
      bundle = await getGameBreakdown({ gamePk, ...opts });
    } catch (e) {
      error = String(e);
      bundle = null;
    } finally {
      loading = false;
    }
    // Records are secondary context — a standings outage shouldn't blank the page.
    try {
      const st = await getStandings();
      const rec = new Map<number, string>();
      for (const t of st.teams) rec.set(t.teamId, `${t.wins}-${t.losses}`);
      recordById = rec;
    } catch {
      /* leave records empty */
    }
  }

  // Matchup context (H2H, splits, lineups, bullpen) loads independently so the
  // model breakdown above never waits on its network calls. Best-effort.
  async function loadContext(gamePk: number) {
    contextLoading = true;
    context = null;
    try {
      context = await getGameContext({ gamePk });
    } catch {
      /* best-effort — leave context null */
    } finally {
      contextLoading = false;
    }
  }

  // Reload whenever the route param or query state changes (SvelteKit reuses this
  // component across game→game navigations, so onMount alone wouldn't refire).
  $effect(() => {
    const gamePk = Number(page.params.gamePk);
    const sp = page.url.searchParams;
    if (!Number.isFinite(gamePk)) {
      error = "Invalid game id.";
      loading = false;
      return;
    }
    const expRaw = sp.get("exp");
    load(gamePk, {
      exponent: expRaw != null ? Number(expRaw) : undefined,
      includePitchers: sp.get("p") !== "false",
      includeHomeField: sp.get("hf") !== "false",
      includeRecentForm: sp.get("rf") !== "false",
    });
    loadContext(gamePk);
  });

  // a = home team, b = away team (set that way in compute_head_to_head).
  function seriesLine(c: GameContextBundle): string {
    const h = c.headToHead;
    if (h.meetings.length === 0) return "First meeting of the season";
    if (h.aWins === h.bWins) return `Series tied ${h.aWins}–${h.bWins}`;
    const name = h.aWins > h.bWins ? c.home : c.away;
    return `${name} lead the series ${Math.max(h.aWins, h.bWins)}–${Math.min(h.aWins, h.bWins)}`;
  }
  const rate = (x: number) => x.toFixed(1);

  function goBack() {
    if (typeof history !== "undefined" && history.length > 1) history.back();
    else goto("/");
  }

  const num = (x: number, d = 2) => x.toFixed(d);
  const record = (teamId: number) => recordById.get(teamId) ?? null;

  // Home scoring uses home offense vs away defense (and vice-versa) — expose the
  // opponent's effective RA so the runs formula reads correctly.
  function oppDs(side: "home" | "away"): number {
    if (!bundle) return 0;
    return side === "home" ? bundle.breakdown.away.dsEff : bundle.breakdown.home.dsEff;
  }
</script>

<section class="wrap">
  <button class="back" onclick={goBack}>← Back to predictions</button>

  {#if loading && !bundle}
    <div class="card center">
      <span class="spinner" aria-hidden="true"></span>
      <p class="muted">Loading matchup breakdown…</p>
    </div>
  {:else if error}
    <div class="card err">
      <strong>Couldn't load this matchup.</strong>
      <p class="mono small">{error}</p>
      <p><a href="/">Return to predictions</a></p>
    </div>
  {:else if bundle}
    {@const b = bundle.breakdown}
    {@const awayWin = bundle.prediction.awayWinProb >= 0.5}
    {@const homeWin = bundle.prediction.homeWinProb >= 0.5}

    <!-- ── HEADER: the answer up top ─────────────────────────── -->
    <header class="head">
      <div class="matchup-title">
        <div class="team-block away">
          <h1>{bundle.away}</h1>
          <span class="sub">Away{#if record(bundle.awayTeamId)} · {record(bundle.awayTeamId)}{/if}</span>
        </div>
        <span class="at">@</span>
        <div class="team-block home">
          <h1>{bundle.home}</h1>
          <span class="sub">Home{#if record(bundle.homeTeamId)} · {record(bundle.homeTeamId)}{/if}</span>
        </div>
      </div>
      <div class="meta">
        <span class="badge">{bundle.date}</span>
        <span class="badge">x = {num(b.exponent, 3)}</span>
        <span class="badge">Lg avg {num(b.leagueAvgRuns, 2)} R/team/g</span>
      </div>
    </header>

    <div class="card summary">
      <div class="probs">
        <div class="prob" class:winner={awayWin}>{fmtPct(bundle.prediction.awayWinProb, 1)}</div>
        <div class="prob" class:winner={homeWin}>{fmtPct(bundle.prediction.homeWinProb, 1)}</div>
      </div>
      <div class="bars">
        <div class="bar bar-away" class:winner={awayWin}>
          <span class="fill" style="--w: {(bundle.prediction.awayWinProb * 100).toFixed(1)}%"></span>
        </div>
        <div class="bar bar-home" class:winner={homeWin}>
          <span class="fill" style="--w: {(bundle.prediction.homeWinProb * 100).toFixed(1)}%"></span>
        </div>
      </div>
      <div class="summary-row">
        <div class="sr-cell">
          <span class="sr-val" class:winner={awayWin}>{fmtOdds(bundle.prediction.awayFairOdds)}</span>
          <span class="sr-lbl">Fair odds</span>
          <span class="sr-val" class:winner={homeWin}>{fmtOdds(bundle.prediction.homeFairOdds)}</span>
        </div>
        <div class="sr-cell">
          <span class="sr-val">{fmtRuns(bundle.prediction.awayPredRuns)}</span>
          <span class="sr-lbl">Projected runs</span>
          <span class="sr-val">{fmtRuns(bundle.prediction.homePredRuns)}</span>
        </div>
        <div class="sr-cell total">
          <span class="sr-lbl">Total</span>
          <span class="sr-val big">{fmtRuns(bundle.prediction.totalRuns)}</span>
        </div>
      </div>
    </div>

    <p class="intro">
      Here's how the model arrived at those numbers — the same steps the
      <a href="/learn">Learn</a> page walks through, filled in with this game's figures.
    </p>

    <!-- ── STEP 1: rates ─────────────────────────────────────── -->
    <section class="step">
      <div class="step-head">
        <span class="step-num">1</span>
        <h2>Scoring &amp; run prevention</h2>
        <InfoTip text="Each team's runs scored per game (RS/G) and runs allowed per game (RA/G). Recent form nudges these toward the last 20 games; the starter's ERA then shifts the RA the team is expected to allow tonight." />
      </div>
      <div class="rate-grid">
        {#each [{ side: "away", name: bundle.away, s: b.away, p: bundle.awayPitcher }, { side: "home", name: bundle.home, s: b.home, p: bundle.homePitcher }] as col}
          {@const s = col.s as SideBreakdown}
          <div class="rate-col">
            <h3>{col.name}</h3>

            <div class="rate-line">
              <span class="rate-tag">RS/G</span>
              <span class="chain">
                <span class="node">season {num(s.seasonRsPerGame)}</span>
                {#if s.recentApplied}
                  <span class="arrow">→</span>
                  <span class="node dim">L{s.recentGames} {num(s.recentRsPerGame)}</span>
                  <span class="arrow">→</span>
                  <span class="node strong">{num(s.blendedRsPerGame)}</span>
                {/if}
              </span>
            </div>

            <div class="rate-line">
              <span class="rate-tag">RA/G</span>
              <span class="chain">
                <span class="node" class:strong={!s.recentApplied && !s.pitcherApplied}>season {num(s.seasonRaPerGame)}</span>
                {#if s.recentApplied}
                  <span class="arrow">→</span>
                  <span class="node dim">L{s.recentGames} {num(s.recentRaPerGame)}</span>
                  <span class="arrow">→</span>
                  <span class="node" class:strong={!s.pitcherApplied}>{num(s.blendedRaPerGame)}</span>
                {/if}
                {#if s.pitcherApplied}
                  <span class="arrow">→</span>
                  <span class="node dim">SP {num(s.pitcherEra)} ERA</span>
                  <span class="arrow">→</span>
                  <span class="node strong">{num(s.effectiveRaPerGame)}</span>
                {/if}
              </span>
            </div>

            <p class="rate-note">
              {#if s.recentApplied && s.pitcherApplied}
                RS/G is a 60/40 season-to-L20 blend; RA/G blends that, then folds in 60% of {col.p?.name ?? "the starter"}'s ERA.
              {:else if s.recentApplied}
                RS/G and RA/G are 60/40 season-to-L20 blends.{#if col.p && !s.pitcherApplied} Starter ERA not applied (off or &lt; 20 IP).{/if}
              {:else if s.pitcherApplied}
                RA/G folds in 60% of {col.p?.name ?? "the starter"}'s ERA.
              {:else}
                Using pure season rates (recent-form and pitcher adjustments off or ineligible).
              {/if}
            </p>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── STEP 2: per-team Pythagorean ──────────────────────── -->
    <section class="step">
      <div class="step-head">
        <span class="step-num">2</span>
        <h2>Each team's Pythagorean win %</h2>
        <InfoTip text="Bill James' Pythagorean expectation converts a team's scoring and run-prevention rates into a standalone winning percentage." />
      </div>
      <Formula label="Pythagorean Expectation">
        <em>Win %</em> =
        <span class="frac">
          <span class="num"><em>RS</em><sup>x</sup></span>
          <span class="den"><em>RS</em><sup>x</sup> + <em>RA</em><sup>x</sup></span>
        </span>
      </Formula>
      <div class="calc-grid">
        <div class="calc-col">
          <span class="calc-team">{bundle.away}</span>
          <span class="calc-line">{num(b.away.blendedRsPerGame)}<sup>{num(b.exponent, 2)}</sup> / ({num(b.away.blendedRsPerGame)}<sup>{num(b.exponent, 2)}</sup> + {num(b.away.effectiveRaPerGame)}<sup>{num(b.exponent, 2)}</sup>)</span>
          <span class="calc-res">{fmtPct(b.away.pythagWinPct, 1)}</span>
        </div>
        <div class="calc-col">
          <span class="calc-team">{bundle.home}</span>
          <span class="calc-line">{num(b.home.blendedRsPerGame)}<sup>{num(b.exponent, 2)}</sup> / ({num(b.home.blendedRsPerGame)}<sup>{num(b.exponent, 2)}</sup> + {num(b.home.effectiveRaPerGame)}<sup>{num(b.exponent, 2)}</sup>)</span>
          <span class="calc-res">{fmtPct(b.home.pythagWinPct, 1)}</span>
        </div>
      </div>
    </section>

    <!-- ── STEP 3: log5 ──────────────────────────────────────── -->
    <section class="step">
      <div class="step-head">
        <span class="step-num">3</span>
        <h2>Combining the two teams (log5)</h2>
        <InfoTip text="log5 turns two standalone win percentages into the probability that one beats the other. This is the neutral-site matchup probability, before any home-field adjustment." />
      </div>
      <Formula label="log5">
        <em>P(home)</em> =
        <span class="frac">
          <span class="num"><em>H</em>(1 − <em>A</em>)</span>
          <span class="den"><em>H</em>(1 − <em>A</em>) + (1 − <em>H</em>)<em>A</em></span>
        </span>
      </Formula>
      <div class="single-calc">
        <span class="calc-line">
          H = {fmtPct(b.home.pythagWinPct, 1)}, A = {fmtPct(b.away.pythagWinPct, 1)}
        </span>
        <span class="calc-res">
          Neutral home win {fmtPct(b.neutralHomeWin, 1)}
        </span>
      </div>
    </section>

    <!-- ── STEP 4: home field ────────────────────────────────── -->
    <section class="step" class:faded={!b.homeFieldApplied}>
      <div class="step-head">
        <span class="step-num">4</span>
        <h2>Home-field advantage</h2>
        <InfoTip text="A fixed shift in log-odds space matching MLB's ~54% historical home win rate. Because it's applied to the log-odds, it bumps a coin-flip game more than a lopsided one." />
      </div>
      {#if b.homeFieldApplied}
        <div class="single-calc">
          <span class="calc-line">
            log-odds({fmtPct(b.neutralHomeWin, 1)}) + {num(b.homeFieldDelta, 4)}
          </span>
          <span class="calc-res">
            {fmtPct(b.neutralHomeWin, 1)} → {fmtPct(b.finalHomeWin, 1)} home win
          </span>
        </div>
      {:else}
        <p class="rate-note">Home-field advantage is off — the neutral-site probability is used as the final win probability.</p>
      {/if}
    </section>

    <!-- ── STEP 5: predicted runs ────────────────────────────── -->
    <section class="step">
      <div class="step-head">
        <span class="step-num">5</span>
        <h2>Predicted runs</h2>
        <InfoTip text="Offensive strength (OS = team RS/G ÷ league avg) times the opponent's defensive strength (DS = their RA/G ÷ league avg), scaled back up by the league average. Home-field advantage does not affect run totals." />
      </div>
      <Formula label="Predicted runs">
        <em>Runs</em> = <em>OS</em> × <em>opponent DS</em> × <em>league avg</em>
      </Formula>
      <div class="calc-grid">
        <div class="calc-col">
          <span class="calc-team">{bundle.away}</span>
          <span class="calc-line">{num(b.away.osEff, 3)} × {num(oppDs("away"), 3)} × {num(b.leagueAvgRuns, 2)}</span>
          <span class="calc-res">{fmtRuns(b.away.predRuns)} runs</span>
        </div>
        <div class="calc-col">
          <span class="calc-team">{bundle.home}</span>
          <span class="calc-line">{num(b.home.osEff, 3)} × {num(oppDs("home"), 3)} × {num(b.leagueAvgRuns, 2)}</span>
          <span class="calc-res">{fmtRuns(b.home.predRuns)} runs</span>
        </div>
      </div>
    </section>

    <!-- ── STEP 6: fair odds ─────────────────────────────────── -->
    <section class="step">
      <div class="step-head">
        <span class="step-num">6</span>
        <h2>Fair odds</h2>
        <InfoTip text="The final win probability converted to American moneyline odds, with no bookmaker margin (vig)." />
      </div>
      <div class="calc-grid">
        <div class="calc-col">
          <span class="calc-team">{bundle.away}</span>
          <span class="calc-line">{fmtPct(bundle.prediction.awayWinProb, 1)}</span>
          <span class="calc-res">{fmtOdds(bundle.prediction.awayFairOdds)}</span>
        </div>
        <div class="calc-col">
          <span class="calc-team">{bundle.home}</span>
          <span class="calc-line">{fmtPct(bundle.prediction.homeWinProb, 1)}</span>
          <span class="calc-res">{fmtOdds(bundle.prediction.homeFairOdds)}</span>
        </div>
      </div>
    </section>

    <!-- ── TIER 2: matchup & team context ────────────────────── -->
    {#snippet splitCol(name: string, s: TeamSplits)}
      <div class="calc-col ctx-col">
        <span class="calc-team">{name}</span>
        <div class="split-row"><span class="split-tag">Home</span><span class="split-val">{s.home.wins}-{s.home.losses} · {rate(s.home.rsPerGame)}/{rate(s.home.raPerGame)} R</span></div>
        <div class="split-row"><span class="split-tag">Road</span><span class="split-val">{s.road.wins}-{s.road.losses} · {rate(s.road.rsPerGame)}/{rate(s.road.raPerGame)} R</span></div>
        {#if s.l10}
          <div class="split-row"><span class="split-tag">L10</span><span class="split-val">{rate(s.l10.rsPerGame)}/{rate(s.l10.raPerGame)} R over {s.l10.games}</span></div>
        {/if}
      </div>
    {/snippet}
    {#snippet lineupCol(name: string, spots: LineupSpot[])}
      <div class="calc-col ctx-col">
        <span class="calc-team">{name}</span>
        {#if spots.length > 0}
          <ol class="lineup">
            {#each spots as sp}
              <li><span class="lu-order">{sp.order}</span><span class="lu-name">{sp.name}</span><span class="lu-pos">{sp.position}</span></li>
            {/each}
          </ol>
        {:else}
          <p class="ctx-empty">Lineup not posted yet.</p>
        {/if}
      </div>
    {/snippet}
    {#snippet bullpenCol(name: string, bp: Bullpen | null)}
      <div class="calc-col ctx-col">
        <span class="calc-team">{name}</span>
        {#if bp}
          <div class="bp-stats">
            <div class="bp-stat"><span class="bp-val">{bp.era.toFixed(2)}</span><span class="bp-lbl">ERA</span></div>
            <div class="bp-stat"><span class="bp-val">{rate(bp.inningsPitched)}</span><span class="bp-lbl">IP</span></div>
            <div class="bp-stat"><span class="bp-val">{bp.whip.toFixed(2)}</span><span class="bp-lbl">WHIP</span></div>
            <div class="bp-stat"><span class="bp-val">{bp.saves}</span><span class="bp-lbl">SV</span></div>
          </div>
        {:else}
          <p class="ctx-empty">Bullpen stats unavailable.</p>
        {/if}
      </div>
    {/snippet}

    {#if contextLoading && !context}
      <section class="step">
        <p class="muted"><span class="spinner" aria-hidden="true"></span> Loading matchup context…</p>
      </section>
    {:else if context}
      <section class="step">
        <div class="step-head"><h2>Season series</h2></div>
        {#if context.headToHead.meetings.length === 0}
          <p class="ctx-empty">These teams haven't met yet this season.</p>
        {:else}
          <p class="series-lead">{seriesLine(context)}</p>
          <p class="muted small">Runs this season — {context.home} {context.headToHead.aRuns}, {context.away} {context.headToHead.bRuns}</p>
          <ul class="meetings">
            {#each context.headToHead.meetings as m}
              <li>
                <span class="m-date">{m.date}</span>
                <span class="m-score">{m.awayName} {m.awayRuns} @ {m.homeName} {m.homeRuns}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section class="step">
        <div class="step-head"><h2>Home / road &amp; recent splits</h2></div>
        <div class="calc-grid">
          {@render splitCol(context.away, context.awaySplits)}
          {@render splitCol(context.home, context.homeSplits)}
        </div>
      </section>

      <section class="step">
        <div class="step-head"><h2>Probable lineups</h2></div>
        <div class="calc-grid">
          {@render lineupCol(context.away, context.lineups.away)}
          {@render lineupCol(context.home, context.lineups.home)}
        </div>
      </section>

      <section class="step">
        <div class="step-head"><h2>Bullpen (relief pitching)</h2></div>
        <div class="calc-grid">
          {@render bullpenCol(context.away, context.awayBullpen)}
          {@render bullpenCol(context.home, context.homeBullpen)}
        </div>
      </section>
    {/if}
  {/if}
</section>

<style>
  .wrap {
    max-width: 900px;
    margin: 0 auto;
  }
  .back {
    background: transparent;
    border: none;
    color: var(--ink-soft);
    font: inherit;
    font-size: 0.9rem;
    padding: 4px 0;
    cursor: pointer;
    margin-bottom: 18px;
  }
  .back:hover { color: var(--ink); }

  .center { text-align: center; padding: 40px 20px; }
  .err { border-color: var(--accent); background: var(--accent-soft); }
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

  /* HEADER */
  .head { margin-bottom: 18px; }
  .matchup-title {
    display: flex;
    align-items: baseline;
    gap: 18px;
    flex-wrap: wrap;
  }
  .team-block h1 {
    font-family: var(--serif);
    font-size: 1.9rem;
    line-height: 1.1;
    margin: 0;
  }
  .team-block .sub {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
  }
  .at { color: var(--ink-mute); font-size: 1.2rem; }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }

  /* SUMMARY */
  .summary { padding: 20px 24px; margin-bottom: 8px; }
  .probs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .prob {
    font-size: 2rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
    line-height: 1;
  }
  .prob:first-child { text-align: left; }
  .prob:last-child { text-align: right; }
  .prob.winner { color: var(--good); }
  .bars {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin: 8px 0 16px;
  }
  .bar {
    height: 14px;
    background: var(--bg-soft);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .bar .fill { display: block; height: 100%; width: var(--w); background: var(--bad); }
  .bar.winner .fill { background: var(--good); }
  .bar-away .fill { margin-right: auto; }
  .bar-home .fill { margin-left: auto; }

  .summary-row {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 18px;
    border-top: 1px solid var(--line-soft);
    padding-top: 14px;
  }
  .sr-cell {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 10px;
    text-align: center;
  }
  .sr-cell.total { grid-template-columns: auto auto; gap: 8px; }
  .sr-val {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    color: var(--ink);
    font-weight: 600;
  }
  .sr-val.big { font-size: 1.2rem; }
  .sr-val.winner { color: var(--good); }
  .sr-lbl {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    white-space: nowrap;
  }

  .intro {
    color: var(--ink-soft);
    margin: 22px 0 8px;
    max-width: 62ch;
  }

  /* STEPS */
  .step {
    border-top: 1px solid var(--line-soft);
    padding: 22px 0 4px;
  }
  .step.faded { opacity: 0.62; }
  .step-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .step-num {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px; height: 26px;
    border-radius: 50%;
    background: var(--ink);
    color: var(--bg-elev);
    font-size: 0.85rem;
    font-weight: 600;
    flex-shrink: 0;
  }
  .step-head h2 {
    font-size: 1.15rem;
    margin: 0;
  }

  .rate-grid,
  .calc-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .rate-col, .calc-col {
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    padding: 14px 16px;
  }
  .rate-col h3 {
    font-family: var(--serif);
    font-size: 1.05rem;
    margin: 0 0 10px;
  }
  .rate-line {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }
  .rate-tag {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    width: 3em;
    flex-shrink: 0;
  }
  .chain {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    flex-wrap: wrap;
    font-family: var(--mono);
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }
  .node { color: var(--ink-soft); }
  .node.dim { color: var(--ink-mute); }
  .node.strong { color: var(--ink); font-weight: 600; }
  .arrow { color: var(--ink-mute); }
  .rate-note {
    font-size: 0.78rem;
    color: var(--ink-mute);
    margin: 6px 0 0;
    line-height: 1.4;
  }

  .calc-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: center;
  }
  .calc-team {
    font-family: var(--serif);
    font-size: 1rem;
    color: var(--ink);
  }
  .calc-line {
    font-family: var(--mono);
    font-size: 0.82rem;
    color: var(--ink-mute);
    font-variant-numeric: tabular-nums;
  }
  .calc-res {
    font-family: var(--mono);
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .single-calc {
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
    text-align: center;
  }

  /* TIER 2 CONTEXT */
  .ctx-col {
    text-align: left;
    gap: 8px;
  }
  .ctx-empty {
    font-size: 0.82rem;
    color: var(--ink-mute);
    font-style: italic;
    margin: 4px 0 0;
  }
  .series-lead {
    font-family: var(--serif);
    font-size: 1.15rem;
    margin: 0 0 4px;
  }
  .meetings {
    list-style: none;
    padding: 0;
    margin: 12px 0 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .meetings li {
    display: flex;
    gap: 12px;
    align-items: baseline;
    font-size: 0.85rem;
  }
  .m-date {
    font-family: var(--mono);
    font-size: 0.75rem;
    color: var(--ink-mute);
    width: 6.5em;
    flex-shrink: 0;
  }
  .m-score { color: var(--ink-soft); }

  .split-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 0.85rem;
  }
  .split-tag {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    width: 3em;
    flex-shrink: 0;
  }
  .split-val {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    color: var(--ink-soft);
  }

  .lineup {
    list-style: none;
    padding: 0;
    margin: 4px 0 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .lineup li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 0.85rem;
  }
  .lu-order {
    font-family: var(--mono);
    color: var(--ink-mute);
    width: 1.4em;
    flex-shrink: 0;
  }
  .lu-name { color: var(--ink); flex: 1; }
  .lu-pos {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-mute);
  }

  .bp-stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    margin-top: 4px;
  }
  .bp-stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .bp-val {
    font-family: var(--mono);
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .bp-lbl {
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-mute);
  }

  @media (max-width: 620px) {
    .rate-grid, .calc-grid { grid-template-columns: 1fr; }
    .summary-row { grid-template-columns: 1fr; }
  }
</style>
