# MLB Pythagorean Expectation

A desktop app — Rust + Tauri — that predicts MLB game outcomes using Bill James' Pythagorean Expectation (with a starting-pitcher adjustment), and teaches you how the math works.

> Originally written in R. This repo contains both the original R scripts (in [`legacy/`](./legacy)) and the new Tauri rewrite.

## What it does

- **Predictions** — every game on a given date as a self-contained card: away/home team names, win-probability bars (green favored / red underdog), projected runs, fair American odds, season rank, R/G and RA/G per team, and the announced starting pitcher with his season ERA.
- **Learn** — an interactive walkthrough of the model: Pythagorean expectation, log5, OS/DS for predicted runs, fair-odds conversion, and the pitcher adjustment. Each section has its own anchor in the left-side TOC.
- **Playground** — a sortable table of all 30 teams (Rank · W% · R/G · RA/G · OS · DS) on the left; pick a Home and Away with one click, then tweak runs, games, the Pythagorean exponent, and an optional Starter ERA / IP on the right. The win-prob, predicted runs, fair odds, and a sensitivity chart update live.
- Data is pulled directly from the public [MLB Stats API](https://statsapi.mlb.com). Schedule cached for 10 minutes, pitcher stats for 1 hour.

## The model in one paragraph

Each team's standalone strength is its *Pythagorean win %* — `RS^x / (RS^x + RA^x)`, where the exponent `x` is fit to the current season's actual results (typically near 1.6–1.9). For any given matchup, if the probable starting pitcher is announced and has at least 20 innings on the season, the team's effective runs-allowed for that game is `0.6 · starter_ERA + 0.4 · team_RA/G` — capturing the fact that the starter handles ~60% of the innings. The two teams' adjusted Pythagorean win %s are combined into a matchup probability via *log5*. Predicted runs use offensive and defensive strength (relative to the league average), where defensive strength uses the pitcher-adjusted RA. A full walkthrough is built into the app's Learn tab.

## Pitcher adjustment toggle

Both the Predictions and Playground pages have a toggle to turn the pitcher adjustment off — useful for comparing the augmented model against the pure Pythagorean baseline, or for falling back when the listed starter is questionable. When off, the math is the team-level Pythagorean it was originally.

## Running it

Prereqs: a recent Rust toolchain, Node 20+, and pnpm.

```bash
pnpm install
pnpm tauri dev          # launch the dev app
pnpm tauri build        # produce a distributable bundle (.dmg / .app / .msi / .deb)
```

The app fetches the current season's schedule on first load — give it a few seconds. Subsequent loads hit the cache.

## How it's wired

```
src-tauri/src/
├── mlb_api.rs     # statsapi.mlb.com client (schedule + people endpoints)
├── model.rs       # Pythagorean, log5, OS/DS, pitcher blend, golden-section exponent fitter
└── lib.rs         # Tauri commands + in-memory caches

src/
├── routes/
│   ├── +page.svelte                # Predictions (cards)
│   ├── learn/+page.svelte          # Educational walkthrough w/ left TOC
│   └── playground/+page.svelte     # Team table + matchup editor
└── lib/
    ├── api.ts                      # invoke() wrappers
    ├── types.ts                    # TS types mirroring Rust structs
    └── format.ts                   # %, odds, CSV export
```

## Verifying changes

The fastest way to validate a backend change against live data:

```bash
cd src-tauri
cargo run --example smoke                # today's slate
cargo run --example smoke 2026 2026-05-23 # specific season + date
```

This hits the real MLB API, runs the full prediction pipeline including pitcher fetch, and prints the resulting prediction table to stdout. Useful for catching regressions in the schedule normalizer or the pitcher blend before opening the GUI.

```bash
cargo test --lib                        # unit tests (pythag, log5, odds, innings parsing)
```

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for the broader feature list — standings, model performance tracker, live scoreboard, park factors, and more. The pitcher adjustment was item #1 and is in.

## Acknowledgments

- **Bill James** for the original Pythagorean Expectation and log5 formulations.
- **[MLB Stats API](https://statsapi.mlb.com)** — public, no auth required, replaces what the R `baseballr` package wraps.
- The original R implementation lives in [`legacy/`](./legacy) for reference.

## License

MIT — see [LICENSE](./LICENSE).
