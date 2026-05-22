# MLB Pythagorean Expectation

A desktop app — Rust + Tauri — that predicts MLB game outcomes using Bill James' Pythagorean Expectation, and teaches you how the math works.

> Originally written in R. This repo contains both the original R scripts (in [`legacy/`](./legacy)) and the new Tauri rewrite.

## What it does

- **Predictions** for today's games (or any date), showing win probabilities, fair American odds, and predicted runs.
- **Learn** page walking through the model — the formulas, why it works, and what it ignores.
- **Playground** with a draggable exponent slider and editable team stats so you can feel how the math responds.
- Data is pulled directly from the public [MLB Stats API](https://statsapi.mlb.com), cached locally for 10 minutes.

## The model in one paragraph

Each team's standalone strength is its *Pythagorean win %* — `RS^x / (RS^x + RA^x)`, where the exponent `x` is fit to the current season's results (typically near 1.83). Two teams are combined into a matchup probability via *log5*. Predicted runs use offensive and defensive strength relative to the league average. A full walkthrough is built into the app's Learn tab.

## Running it

Prereqs: a recent Rust toolchain, Node 20+, and pnpm.

```bash
pnpm install
pnpm tauri dev          # launch the dev app
pnpm tauri build        # produce a distributable bundle (.dmg / .app / .msi / .deb depending on platform)
```

The app fetches the current season's schedule on first load — give it a few seconds.

## How it's wired

```
src-tauri/src/
├── mlb_api.rs     # statsapi.mlb.com client (replaces the R `baseballr` calls)
├── model.rs       # Pythagorean, log5, OS/DS, golden-section exponent fitter
└── lib.rs         # Tauri commands + in-memory cache

src/
├── routes/
│   ├── +page.svelte           # Predictions
│   ├── learn/+page.svelte     # Educational walkthrough
│   └── playground/+page.svelte# Interactive sliders + sensitivity chart
└── lib/
    ├── api.ts                 # invoke() wrappers
    ├── types.ts               # TS types mirroring Rust structs
    └── format.ts              # Odds, percentages, CSV export
```

## Acknowledgments

- **Bill James** for the original Pythagorean Expectation and log5 formulations.
- **[MLB Stats API](https://statsapi.mlb.com)** — public, no auth required, replaces what the R `baseballr` package wraps.
- The original R implementation lives in [`legacy/`](./legacy) for reference.

## License

MIT — see [LICENSE](./LICENSE).
