<script lang="ts">
  import Formula from "$lib/components/Formula.svelte";

  const toc = [
    { id: "idea", label: "1 · The idea" },
    { id: "why", label: "2 · Why it works" },
    { id: "log5", label: "3 · Combining two teams" },
    { id: "score", label: "4 · Predicting the score" },
    { id: "odds", label: "5 · Fair odds" },
    { id: "example", label: "6 · Worked example" },
    { id: "limits", label: "7 · What it doesn't do" },
    { id: "pitcher", label: "8 · The pitcher adjustment" },
    { id: "homefield", label: "9 · Home-field advantage" },
  ];
</script>

<div class="layout">
  <aside class="toc" aria-label="On this page">
    <p class="toc-title">On this page</p>
    <ul>
      {#each toc as item}
        <li><a href={`#${item.id}`}>{item.label}</a></li>
      {/each}
    </ul>
  </aside>

  <article class="learn">
    <header>
      <h1>What is Pythagorean Expectation?</h1>
      <p class="lede">
        A 40-year-old idea from baseball stats that says: <em>the runs a team scores and allows tell you most of what you need to know about how often it wins.</em>
        Everything in this app — win probabilities, predicted scores, fair odds — falls out of that.
      </p>
    </header>

  <section id="idea">
    <h2>1 · The idea</h2>
    <p>
      In 1980, Bill James (the sabermetrician behind <em>Moneyball</em>'s intellectual roots) was looking for a simple way to estimate
      a team's expected winning percentage. He noticed that
      a team's record over a long stretch tracks closely with a function of runs scored and runs allowed that looks a lot like
      the Pythagorean theorem — hence the name.
    </p>

    <Formula label="Pythagorean Expectation">
      <em>Win %</em> =
      <span class="frac">
        <span class="num"><em>RS</em><sup>x</sup></span>
        <span class="den"><em>RS</em><sup>x</sup> + <em>RA</em><sup>x</sup></span>
      </span>
    </Formula>

    <p>
      <em>RS</em> = runs scored, <em>RA</em> = runs allowed, and <em>x</em> is an exponent.
      James used <span class="mono">x = 2</span> originally (the squares are why it's "Pythagorean").
      Decades of analysis have found that a slightly lower value — typically around <span class="mono">1.83</span> — fits better
      in the modern run-scoring environment. This app re-fits the exponent every time you load it,
      so it stays accurate as the season unfolds.
    </p>
  </section>

  <section id="why">
    <h2>2 · Why it works</h2>
    <p>
      Run differential is a noisier-than-you-think predictor of wins because a single blowout
      affects scoring totals more than it affects the W-L column.
      Squaring runs in the formula pulls the prediction back toward 50/50 when totals are close, and pushes it
      hard toward the better team when totals diverge — matching how actual win/loss records behave.
    </p>
    <p class="subtle">
      A team that has scored exactly as many runs as it has allowed gets a Pythagorean win % of <strong>50%</strong>,
      regardless of the exponent. A team scoring twice as many as it allows pushes up to <strong>~80%</strong> at x = 2.
    </p>
  </section>

  <section id="log5">
    <h2>3 · Combining two teams (log5)</h2>
    <p>
      Pythagorean gives each team a standalone strength, but we need to predict <em>a specific matchup</em>.
      The bridge is <strong>log5</strong>, also from Bill James:
    </p>

    <Formula label="log5 probability that team A beats team B">
      <em>P(A beats B)</em> =
      <span class="frac">
        <span class="num"><em>p<sub>A</sub></em>(1 − <em>p<sub>B</sub></em>)</span>
        <span class="den"><em>p<sub>A</sub></em>(1 − <em>p<sub>B</sub></em>) + (1 − <em>p<sub>A</sub></em>)<em>p<sub>B</sub></em></span>
      </span>
    </Formula>

    <p>
      <em>p<sub>A</sub></em> and <em>p<sub>B</sub></em> are each team's Pythagorean win %. The formula gives the right answer
      for the easy cases — a .700 team vs a .500 team comes out near .700; two .500 teams come out at .500 —
      and it generalizes smoothly to the in-between.
    </p>
  </section>

  <section id="score">
    <h2>4 · Predicting the score</h2>
    <p>
      Win probability tells us <em>who</em>, but not <em>by how much</em>. To predict runs, we score each team's offense and defense
      relative to the league average.
    </p>

    <Formula label="Offensive and Defensive Strength">
      <em>OS</em> = (<em>RS</em>/<em>G</em>) / <em>league avg runs</em>
      &nbsp;&nbsp;·&nbsp;&nbsp;
      <em>DS</em> = (<em>RA</em>/<em>G</em>) / <em>league avg runs</em>
    </Formula>

    <p>
      A team that scores 5.0 runs per game in a league averaging 4.5 has <em>OS</em> = 1.11.
      A team that gives up 4.0 in the same league has <em>DS</em> = 0.89 — better defense than average.
      Predicted home runs in a matchup is just home offense × away defense × the run environment:
    </p>

    <Formula label="Expected runs in a matchup">
      <em>E[Home Runs]</em> = <em>OS<sub>home</sub></em> × <em>DS<sub>away</sub></em> × <em>league avg runs</em>
    </Formula>

    <p>
      Total runs is the sum of both sides. Note this implicitly assumes home / away splits balance out across the season,
      which is roughly true at the team level but not at the individual game level.
    </p>
  </section>

  <section id="odds">
    <h2>5 · From probability to fair odds</h2>
    <p>
      Sportsbooks quote prices in American odds: a favorite gets a minus sign (<span class="mono">-150</span>) and the
      underdog a plus sign (<span class="mono">+130</span>). The conversion from a probability <em>p</em> is:
    </p>

    <Formula label="Probability to American odds">
      if <em>p</em> &gt; 0.5: <em>odds</em> = −100 × <em>p</em> / (1 − <em>p</em>)
      &nbsp;&nbsp;·&nbsp;&nbsp;
      if <em>p</em> ≤ 0.5: <em>odds</em> = (1 − <em>p</em>) × 100 / <em>p</em>
    </Formula>

    <p>
      We call this <em>fair</em> odds because they're the "no-vig" price — the line a sportsbook would set if it weren't taking
      a cut. Compare these to the actual market odds, and the gap is roughly the book's edge for that game.
    </p>
  </section>

  <section id="example">
    <h2>6 · Worked example</h2>
    <div class="example">
      <p>
        Suppose at this point in the season the Dodgers have scored 540 runs and allowed 410 over 100 games,
        and the Marlins have scored 400 and allowed 510 over 99 games. The league averages 4.50 runs per team per game.
      </p>
      <ol>
        <li>
          <strong>Pythagorean win % (x = 1.83)</strong><br />
          Dodgers: 540<sup>1.83</sup> / (540<sup>1.83</sup> + 410<sup>1.83</sup>) ≈ <strong>0.624</strong><br />
          Marlins: 400<sup>1.83</sup> / (400<sup>1.83</sup> + 510<sup>1.83</sup>) ≈ <strong>0.395</strong>
        </li>
        <li>
          <strong>log5 (Dodgers home)</strong><br />
          0.624 × (1 − 0.395) / [0.624 × (1 − 0.395) + (1 − 0.624) × 0.395] ≈ <strong>71.7%</strong>
          → fair odds ~<span class="mono">-253</span>
        </li>
        <li>
          <strong>Predicted runs</strong><br />
          OS<sub>LAD</sub> = (540/100) / 4.50 = 1.20 · DS<sub>MIA</sub> = (510/99) / 4.50 = 1.144<br />
          E[LAD] = 1.20 × 1.144 × 4.50 ≈ <strong>6.2</strong>; E[MIA] ≈ 3.4; total ≈ <strong>9.6</strong>
        </li>
      </ol>
    </div>
  </section>

  <section id="limits">
    <h2>7 · What this model doesn't do</h2>
    <ul>
      <li>It doesn't know <strong>who's pitching</strong>. The 2025 ace vs. a spot starter is invisible to season-long aggregates.</li>
      <li>It doesn't know <strong>injuries, rest, or lineups</strong> — just team totals to date.</li>
      <li>It treats home/away as equivalent — there's a real but small home-field edge in MLB (~54%) we don't model.</li>
      <li>It weights April and September equally — no recency weighting. Hot/cold streaks are smoothed out.</li>
      <li>It assumes no <strong>park factors</strong> — Coors Field and Petco are the same to the model.</li>
    </ul>
    <p>
      Despite all of that, the Pythagorean baseline is famously hard to beat. It's the floor every more complex MLB
      prediction model has to clear, and it's the right starting point to build intuition.
    </p>
  </section>

  <section id="pitcher">
    <h2>8 · The pitcher adjustment</h2>
    <p>
      Pure Pythagorean expectation is blind to who's on the mound. A team's RA/G is the
      <em>average</em> across every starter they've used — but tonight, one specific arm is throwing.
      That's usually the single biggest per-game factor the base model misses.
    </p>
    <p>
      We fix it by replacing each team's effective runs-allowed rate for the matchup with a blend:
    </p>

    <Formula label="Effective RA for tonight's matchup">
      <em>RA<sub>eff</sub></em>/G  =  <em>w</em> · <em>pitcher</em><sub>ERA</sub>
      +  (1 − <em>w</em>) · <em>team</em><sub>RA/G</sub>
    </Formula>

    <p>
      Where <em>w</em> = <strong>0.6</strong>. The starter pitches roughly 5.4 of 9 innings on
      average — about 60% of the game — so they're responsible for ~60% of the runs allowed.
      The remaining 40% is the bullpen and the rest of the team's defense, which is implicitly
      captured by the team's season RA/G.
    </p>

    <p>
      Once we have <em>RA<sub>eff</sub></em>, we re-run the Pythagorean math with the team's
      <em>unchanged</em> RS/G (offense isn't affected by who's pitching for them) and this game-specific
      effective RA. The new standalone win % flows into log5 as before, and the
      run-prediction math uses the matchup-specific DS so an ace suppresses the opposing
      offense's expected runs.
    </p>

    <p class="subtle">
      <strong>Sample-size guardrail:</strong> if the listed starter has fewer than <span class="mono">20</span>
      innings on the season, we ignore their ERA and fall back to pure team RA. A spot starter
      with 4 IP and an 18.00 ERA shouldn't crater their team's prediction.
    </p>

    <p class="subtle">
      <strong>When the starter is TBD:</strong> many teams don't announce until game day. For
      those games we skip the adjustment entirely — the prediction is pure team-level Pythagorean,
      same as before. You'll see <em>"Starter TBD"</em> on the matchup card.
    </p>

    <p>
      In the <a href="/playground">Playground</a>, each side now has optional Starter ERA / IP
      inputs so you can answer questions like <em>"what if the Dodgers throw their #5 instead of their ace?"</em>
      Leave the fields blank for no adjustment.
    </p>

    <p class="subtle">
      <strong>What we're not modeling:</strong> a starter's recent form (last 5 starts vs. season),
      FIP instead of ERA (FIP is often more predictive in small samples), home/road pitcher splits,
      handedness vs. lineup. All defensible future additions.
    </p>
  </section>

  <section id="homefield">
    <h2>9 · Home-field advantage</h2>
    <p>
      Pure log5 returns the probability one team beats another on a <em>neutral</em> field —
      it has no notion of who's hosting. In reality, MLB home teams win about
      <strong>54%</strong> of all games. Some of that is real (familiarity, sleep, batter's eye, no
      flight the night before); some is umpire bias; some is selection (interleague schedules
      and unbalanced divisions). Whatever the cause, the effect is consistent across the league
      and across decades, and ignoring it leaves money on the table.
    </p>

    <p>
      We apply the bump in <strong>log-odds space</strong> rather than directly to the probability.
      The math:
    </p>

    <Formula label="Home-field advantage as a log-odds shift">
      <em>log-odds(home_win<sub>adj</sub>)</em> =
      <em>log-odds(home_win<sub>neutral</sub>)</em> + 0.1603
    </Formula>

    <p>
      The constant <span class="mono">0.1603</span> is the log-odds difference between 0.50 and
      0.54 — exactly the shift needed to convert a coin-flip game on a neutral field into the
      54% home-team win rate observed historically.
    </p>

    <p>
      Why log-odds instead of just adding 4% to the probability? Because a flat <em>+4%</em>
      over-corrects at the extremes. A 95% favorite at home doesn't gain another 4% of win probability
      — there isn't 4% left to gain in any meaningful sense. The log-odds shift naturally shrinks
      the bump as you approach 0 or 1:
    </p>

    <p class="subtle">
      <strong>Effect of the shift at various baselines:</strong><br />
      &nbsp;&nbsp;50% &rarr; <strong>54.0%</strong> (+4.0 pts) — the design point<br />
      &nbsp;&nbsp;60% &rarr; <strong>63.9%</strong> (+3.9 pts)<br />
      &nbsp;&nbsp;75% &rarr; <strong>78.3%</strong> (+3.3 pts)<br />
      &nbsp;&nbsp;90% &rarr; <strong>91.6%</strong> (+1.6 pts)
    </p>

    <p>
      Both the <a href="/">Predictions</a> page and the <a href="/playground">Playground</a> have a
      <em>Home Field</em> toggle. Flip it off to see the underlying neutral-site probability — useful
      for sanity-checking, comparing against neutral-site models, or evaluating one team's standalone
      strength.
    </p>

    <p class="subtle">
      <strong>What this captures and what it doesn't:</strong> we model the <em>league-average</em>
      home-field effect uniformly. Some parks are tougher to play in than others (think Coors's altitude,
      Fenway's wall, Tropicana's lighting) but we don't differentiate. A per-park multiplier would be a
      reasonable v2.
    </p>
  </section>

  <section class="next">
    <p>
      Want to feel how the math behaves? Head to the <a href="/playground">Playground</a> — drag the exponent,
      edit the teams, watch the win % move.
    </p>
  </section>
  </article>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 64px;
    align-items: start;
  }
  .learn {
    min-width: 0;
  }
  .toc {
    position: sticky;
    top: 90px;
    font-size: 0.88rem;
    border-right: 1px solid var(--line-soft);
    padding-right: 18px;
  }
  .toc-title {
    margin: 0 0 10px;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-mute);
    font-weight: 600;
  }
  .toc ul {
    list-style: none;
    padding: 0;
    margin: 0;
    color: var(--ink-soft);
  }
  .toc li {
    margin: 0;
  }
  .toc a {
    display: block;
    padding: 5px 0;
    color: var(--ink-soft);
    text-decoration: none;
    line-height: 1.35;
  }
  .toc a:hover {
    color: var(--ink);
  }
  @media (max-width: 980px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    .toc {
      display: none;
      border-right: none;
      padding-right: 0;
    }
  }
  .lede {
    font-size: 1.1rem;
    color: var(--ink-soft);
  }
  section {
    margin: 2rem 0;
  }
  .example {
    background: var(--bg-soft);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    padding: 18px 22px;
  }
  .example ol {
    padding-left: 1.2em;
  }
  .example li {
    margin: 0.6em 0;
    color: var(--ink-soft);
  }
  .example li strong {
    color: var(--ink);
  }
  ul {
    color: var(--ink-soft);
    padding-left: 1.2em;
  }
  ul li {
    margin: 0.45em 0;
  }
  .next {
    margin-top: 3rem;
    padding-top: 1.5rem;
    border-top: 1px solid var(--line-soft);
  }
</style>
