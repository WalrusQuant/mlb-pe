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
    { id: "pitcher", label: "8 · The next-start pitcher" },
    { id: "homefield", label: "9 · Home-field advantage" },
    { id: "recent", label: "10 · Recent form weighting" },
    { id: "race", label: "11 · Magic number & ROS walk" },
    { id: "track", label: "12 · Track record" },
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
          Dodgers: 540<sup>1.83</sup> / (540<sup>1.83</sup> + 410<sup>1.83</sup>) ≈ <strong>0.623</strong><br />
          Marlins: 400<sup>1.83</sup> / (400<sup>1.83</sup> + 510<sup>1.83</sup>) ≈ <strong>0.391</strong>
        </li>
        <li>
          <strong>log5 (Dodgers home)</strong><br />
          0.623 × (1 − 0.391) / [0.623 × (1 − 0.391) + (1 − 0.623) × 0.391] ≈ <strong>72.0%</strong>
          → fair odds ~<span class="mono">-257</span>
        </li>
        <li>
          <strong>Predicted runs</strong><br />
          OS<sub>LAD</sub> = (540/100) / 4.50 = 1.200 · DS<sub>MIA</sub> = (510/99) / 4.50 = 1.145<br />
          OS<sub>MIA</sub> = (400/99) / 4.50 = 0.898 · DS<sub>LAD</sub> = (410/100) / 4.50 = 0.911<br />
          E[LAD] = 1.200 × 1.145 × 4.50 ≈ <strong>6.2</strong>; E[MIA] = 0.898 × 0.911 × 4.50 ≈ 3.7; total ≈ <strong>9.9</strong>
        </li>
      </ol>
    </div>
  </section>

  <section id="limits">
    <h2>7 · What this model doesn't do</h2>
    <ul>
      <li>The <em>base</em> Pythagorean number doesn't know <strong>who's pitching</strong>. Section 8 layers a next-start projection on top.</li>
      <li>It doesn't know <strong>injuries, rest, or lineups</strong> — just team totals to date.</li>
      <li>It assumes no <strong>park factors</strong> — Coors Field and Petco are the same to the model.</li>
    </ul>
    <p class="subtle">
      Sections 8, 9, and 10 cover the three optional adjustments we layer on top of the base model
      to address most of the gaps above: the starting pitcher, home-field advantage, and recent form.
    </p>
    <p>
      Despite all of that, the Pythagorean baseline is famously hard to beat. It's the floor every more complex MLB
      prediction model has to clear, and it's the right starting point to build intuition.
    </p>
  </section>

  <section id="pitcher">
    <h2>8 · The next-start pitcher</h2>
    <p>
      Pure Pythagorean expectation is blind to who's on the mound. A team's RA/G is the
      <em>average</em> across every starter they've used — but tonight, one specific arm is throwing.
      Season ERA is a weak stand-in for that: it's earned runs only, it includes the defense
      behind the pitcher (which the 40% team RA already carries), and five recent starts is
      not a sample you should treat as a point estimate.
    </p>
    <p>
      For announced starters we score a <strong>next-start projection</strong> inside this app
      (same public MLB game logs, a frozen Bayesian linear model). It produces two numbers the
      team model actually needs, plus a skill readout:
    </p>
    <ul>
      <li><strong>Projected FIP</strong> — defense-independent skill for the outing. Shown on the card; not blended into RA/G.</li>
      <li><strong>Expected earned runs</strong> and <strong>expected innings</strong> — the outing itself.</li>
    </ul>

    <Formula label="Effective RA for tonight's matchup">
      <em>RA<sub>eff</sub></em>/G  =  E[ER]
      +  max(0, 1 − E[IP]/9) · <em>team</em><sub>RA/G</sub>
    </Formula>

    <p>
      Worked numbers: 2.3 expected ER in 5.3 IP, team RA/G 4.5 →
      <span class="mono">2.3 + (1 − 5.3/9) × 4.5 = 4.15</span>.
      The remaining innings are the bullpen and the rest of the staff, still priced at the
      team's (possibly L20-blended) RA/G. Once we have <em>RA<sub>eff</sub></em>, Pythagorean
      and log5 run as before, and the run-prediction math uses the matchup-specific DS so an
      ace suppresses the opposing offense's expected runs.
    </p>

    <p class="subtle">
      <strong>Fallback:</strong> if the starter is TBD, has no prior starts, or the slate date
      is on or before the frozen model's training cutoff, we use the old blend —
      <span class="mono">0.6 · season ERA + 0.4 · team RA/G</span> — and only if season IP ≥ 20.
      A spot starter with 4 IP and an 18.00 ERA still shouldn't crater the prediction.
    </p>

    <p class="subtle">
      <strong>Honest leftover:</strong> the next-start runs are <em>earned</em>; team RA/G is
      <em>all</em> runs. Same class of mismatch as blending ERA with RA/G. The outing total is
      still the right unit for the starter's share of the game.
    </p>

    <p>
      In the <a href="/playground">Playground</a>, each side has optional next-start ER / IP
      (the outing formula) and season ERA / IP (the fallback). Next-start wins when both
      outing fields are set.
    </p>

    <p class="subtle">
      <strong>Limits:</strong> the Division Race still walks remaining games with pitcher
      blend off — most of those starters aren't announced. Confidence is a reliability tier,
      not “is he good”; the current artifact withholds High. Park in the projection is a
      home/away runs-per-game proxy, not a full park-factor model.
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
      &nbsp;&nbsp;60% &rarr; <strong>63.8%</strong> (+3.8 pts)<br />
      &nbsp;&nbsp;75% &rarr; <strong>77.9%</strong> (+2.9 pts)<br />
      &nbsp;&nbsp;90% &rarr; <strong>91.4%</strong> (+1.4 pts)
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

  <section id="recent">
    <h2>10 · Recent form weighting</h2>
    <p>
      Pythagorean is happy to tell you that the team that won 12 of its last 14 games is no
      better than its full-season totals say it is — which is, on average, true. But it's not
      <em>always</em> true. A team in genuine ascent (rookies clicking, a returned-from-IL ace,
      a new manager) or in real decline (deadline sell-off, key injuries) tells a story April–September
      blurs out. Recent form is how we let the past few weeks tip the scale a little.
    </p>

    <p>
      Each team's effective per-game scoring and prevention rates become weighted averages of the
      season total and the team's <strong>last 20 completed games</strong>:
    </p>

    <Formula label="Recent-form blend (per team, per side)">
      <em>RS/G<sub>eff</sub></em>  =  0.4 · <em>RS/G<sub>L20</sub></em>
      +  0.6 · <em>RS/G<sub>season</sub></em>
      <br />
      <em>RA/G<sub>eff</sub></em>  =  0.4 · <em>RA/G<sub>L20</sub></em>
      +  0.6 · <em>RA/G<sub>season</sub></em>
    </Formula>

    <p>
      A 60/40 season-heavy split keeps the larger sample doing most of the work — the full-season
      number is still the gravitational center — but it lets the L20 line nudge the prediction by a
      visible amount when a team is running noticeably hot or cold. The blended rates feed into
      the Pythagorean math <em>before</em> the pitcher adjustment runs, so the next-start
      outing (or ERA fallback) blends against the recency-aware team baseline, not the
      bare season number.
    </p>

    <p class="subtle">
      <strong>Sample-size guardrail:</strong> if a team has fewer than <span class="mono">10</span>
      completed games (early season, an expansion-team scenario, etc.) we ignore the L20 line
      and use pure season rates. Same idea as the 20-IP guardrail on starters — a 5-game sample
      shouldn't swing a prediction by 40%.
    </p>

    <p>
      Each game card on the <a href="/">Predictions</a> page shows the L20 line beneath the
      starting pitcher: <span class="mono">L20: 5.1 R/G · 3.8 RA/G</span>. When you flip the
      <em>Recent Form</em> toggle off, the line stays visible (so you can see <em>what</em> the
      model is no longer using) but its influence on the win % drops to zero.
    </p>

    <p class="subtle">
      <strong>What this captures and what it doesn't:</strong> the L20 window is a blunt instrument.
      It doesn't distinguish a real talent shift (a healthy ace coming back) from a noisy run
      against weak opponents. Stronger versions would weight games by opponent quality, decay weights
      smoothly over time, or detect the moment a team's underlying talent actually changed. For an
      app whose whole appeal is intelligible math, a flat 60/40 blend with a fixed window felt like
      the right place to stop.
    </p>
  </section>

  <section id="race">
    <h2>11 · Magic number and the rest-of-season walk</h2>
    <p>
      The <a href="/race">Division Race</a> tab splits “who wins this division?” into
      <em>need</em> and <em>supply</em>. Need is the magic number. Supply is what the remaining
      schedule is expected to produce, given each team's Pythagorean talent.
    </p>

    <Formula label="Magic number vs a specific opponent">
      <em>MN</em> = <em>G</em> + 1 − <em>W</em> − <em>L</em><sub>opp</sub>
    </Formula>

    <p>
      <em>G</em> is 162, or the team's actual scheduled length (played + remaining) when that
      isn't 162. Each of your remaining wins and each of the opponent's remaining losses
      knocks one off the number. A head-to-head win is worth <strong>two</strong> — you gain a
      win and they take a loss in the same game. The number we surface is vs the
      <em>closest threat</em> (the opponent who still requires the most of those units).
    </p>

    <p>
      If the season series between the two teams is already complete and you lead it, the
      <span class="mono">+1</span> drops: a tie at the end of the year would go to you on the
      first MLB tiebreaker, so you only need to not finish behind them. If they still play
      each other, we keep the standard formula — the series isn't decided yet. Later
      tiebreakers (intradivision record, and so on) are not applied.
    </p>

    <p>
      Supply is a walk of every remaining game, including out-of-division and interleague
      opponents. Talent is the same Pythagorean W% from RS/RA (and the same fitted exponent)
      used everywhere else in the app. The pitcher blend is off — most remaining starters
      aren't announced. Home-field and recent-form follow the toggles on the Race page.
      Each remaining game's win probability is log5 over those two Pythagorean strengths
      (plus the home-field shift when that toggle is on).
    </p>

    <Formula label="Expected remaining wins">
      <em>E[remaining wins]</em> = Σ <em>P</em>(win game <em>i</em>)
    </Formula>

    <p>
      That's the expected-value layer: a 0.62 game is 0.62 expected wins, not a coin flip
      rounded to 1. Projected final W-L is current record plus that remainder.
    </p>

    <p>
      The second layer is a Monte Carlo. Each remaining game is drawn as a Bernoulli from
      its log5 <em>p</em>, thousands of full season paths. A path counts as a division win
      only if that team uniquely finishes first, or (on a wins tie) uniquely leads the
      season series among the tied group. A simulated tie that head-to-head cannot break
      is not a unique division win — that's why the P(win) column may not sum to 100%.
    </p>
  </section>

  <section id="track">
    <h2>12 · Track record</h2>
    <p>
      The <a href="/track">Track</a> tab pulls every finished game this season from the MLB
      schedule and re-runs the model, then compares the pick and predicted score to the box
      score. For a game on date <em>D</em>, team stats, the fitted exponent, L20, and starter
      ERA use only games <em>before</em> D — same-day results never leak into the prediction.
    </p>
    <p>
      <strong>Right winner</strong> is how often the side with p ≥ 50% actually won.
      <strong>Score error</strong> is mean absolute error on the combined run total (and on
      each side). <strong>Brier</strong> is the mean squared error of the probability (0.25 is
      a coin flip; lower is better). Opening-week games are skipped until both clubs have 10
      completed games. Live <a href="/">Predictions</a> uses <strong>Overdisp</strong> for
      win %: negative binomial (size 2) on the predicted score after shrinking each side 40%
      toward league average. Track A/Bs the wired switches (including Overdisp and Shrink OS/DS).
    </p>
  </section>

  <section class="next">
    <p>
      Want to feel how the math behaves? Head to the <a href="/playground">Playground</a> — drag the exponent, edit the teams, watch the win % move. Or open the <a href="/race">Division Race</a> tab and walk a remaining schedule.
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
