---
title: MLB Pythagorean Expectation
description: A desktop app that predicts MLB games using Bill James' Pythagorean formula
---

A desktop app that predicts MLB games using a 40-year-old formula from baseball stats — Bill James' **Pythagorean Expectation** — and explains the math along the way.

![Predictions screen](./predictions.png)

## What it does

**Predicts every game on the schedule.** For any date, you get a card per matchup with the win probability, projected runs, and fair betting odds.

**Shows you the math.** A built-in *Learn* tab walks through Pythagorean expectation, log5, the run environment, and three optional adjustments — starting pitcher, home-field advantage, and recent form.

**Goes beyond box scores.** A *Stats* tab surfaces leaderboards you won't find on mlb.com: which teams are over- or under-performing their run differential, which are running hot or cold, which have the best offense or defense indexed to the league.

**Lets you play.** A *Playground* tab puts every team's numbers in your hands — drag the Pythagorean exponent, swap pitchers, toggle adjustments, and watch the win probability move in real time.

## A tour

### Standings

Six division tables plus the wild-card race per league, direct from MLB's stats API. Division leaders get a star. Streaks and run differentials are color-coded so the table tells a story at a glance.

![Standings](./standings.png)

### Stats

Every leaderboard here is filtered through the Pythagorean lens. *Luck* compares each team's actual record to what their run differential predicts — Tampa winning more than their numbers suggest, Detroit winning fewer. *Best offenses* and *Best defenses* are indexed to the league average so cross-era comparisons make sense. *Hot and Cold* surfaces real momentum shifts by comparing the last 20 games' net runs to the full season.

![Stats](./stats.png)

### Learn

A walkthrough of the math in plain English. Each section has its own anchor on the left so you can jump to the part you care about — what Pythagorean is, why it works, how we combine two teams into a matchup, how we predict the score, where the numbers come from, and what the model deliberately ignores.

![Learn](./learn.png)

### Playground

A sortable team table on the left, a live matchup editor on the right. Override any team's runs scored or runs allowed, swap in a different starting pitcher's ERA, plug in a custom L20 line, flip any of the three adjustments on or off — the win probability, predicted runs, and fair odds update instantly. The sensitivity chart shows how much the prediction changes as you sweep the Pythagorean exponent.

![Playground](./playground.png)

## How the math works (the short version)

Each team's standalone strength is its **Pythagorean win percentage**:

> Win % = RS<sup>x</sup> / (RS<sup>x</sup> + RA<sup>x</sup>)

where RS is runs scored, RA is runs allowed, and *x* is an exponent the app fits to this season's actual results (usually somewhere near 1.83). Two teams' Pythagorean percentages are combined into a matchup probability via **log5**, a related Bill James formula. From there we layer on optional adjustments — the announced starting pitcher's ERA shifts each side's effective runs allowed, a home-field shift in log-odds space bumps the host's win probability to match MLB's historical ~54% home win rate, and a 60/40 blend with each team's L20 (last 20 games) form lets hot and cold streaks nudge the prediction without overwhelming the season sample.

Predicted runs come from a separate but related idea — each team's offense and defense indexed to the league average, multiplied together.

The *Learn* tab in the app walks through every step with formulas and a worked example.

## Try it

This is currently distributed as source. If you're comfortable with a command line, head to [the GitHub repo](https://github.com/WalrusQuant/mlb-pe) for build instructions — you'll need Rust, Node, and pnpm installed. Prebuilt installers for macOS, Windows, and Linux are on the roadmap.

## Behind it

The app is a [Rust + Tauri](https://tauri.app) rewrite of an older R implementation. Data comes from MLB's public [Stats API](https://statsapi.mlb.com) — no scraping, no auth required. The whole prediction pipeline runs locally on your machine.

The math is from [Bill James](https://en.wikipedia.org/wiki/Bill_James) (Pythagorean expectation, 1980; log5). Everything else is just careful plumbing around it.

[View the source on GitHub →](https://github.com/WalrusQuant/mlb-pe)
