# MLB Pythagorean Expectation — Tauri App Plan

**Goal:** Turn the existing R model (`mlb_pe_spread.R`, `mlb_pe_spread_optim.R`) into a distributable desktop app that (a) shows today's MLB predictions and (b) teaches users what Pythagorean Expectation is and lets them play with the math.

**Stack decisions (confirmed with user):**
- Backend: pure Rust port (no R/Python dependency)
- Frontend: Svelte (via Vite, TypeScript)
- Educational depth: interactive playground

---

## Target architecture

```
mlb-pe/
├── src-tauri/                 # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── icons/
│   └── src/
│       ├── main.rs            # Tauri entrypoint, command registry
│       ├── mlb_api.rs         # statsapi.mlb.com client (schedule + scores)
│       ├── model.rs           # Pythagorean math, log5, optimization
│       ├── commands.rs        # #[tauri::command] handlers
│       └── state.rs           # Cached schedule/team-stats
├── src/                       # Svelte frontend
│   ├── App.svelte             # Layout + nav
│   ├── main.ts
│   ├── lib/
│   │   ├── tauri.ts           # invoke() wrappers w/ typed responses
│   │   ├── types.ts
│   │   ├── format.ts          # odds, %, runs formatters
│   │   └── components/
│   │       ├── PredictionTable.svelte
│   │       ├── TeamCard.svelte
│   │       ├── Formula.svelte         # KaTeX-rendered formulas
│   │       └── Slider.svelte
│   └── routes/
│       ├── Predictions.svelte
│       ├── Learn.svelte
│       └── Playground.svelte
├── legacy/                    # Original R scripts moved here for reference
│   ├── mlb_pe_spread.R
│   └── mlb_pe_spread_optim.R
├── index.html
├── package.json
├── vite.config.ts
└── README.md                  # Updated for Rust/Tauri
```

---

## Phase 1 — Scaffold the project

- [ ] Move R scripts to `legacy/` (preserved but out of the way)
- [ ] Run `npm create tauri-app@latest` with: Svelte + TypeScript + Vite, pnpm
- [ ] Install Tauri CLI (`pnpm add -D @tauri-apps/cli`)
- [ ] Verify `pnpm tauri dev` launches a blank window
- [ ] Wire base layout: top nav with 3 tabs (Predictions / Learn / Playground)

## Phase 2 — Rust backend: MLB Stats API client

`baseballr::mlb_schedule()` is a wrapper around `https://statsapi.mlb.com/api/v1/schedule`. We hit it directly.

- [ ] Add deps to `Cargo.toml`: `reqwest` (rustls), `serde`, `serde_json`, `chrono`, `tokio`, `anyhow`, `thiserror`
- [ ] `mlb_api.rs`:
  - [ ] `fetch_schedule(season: i32) -> Result<Vec<Game>>` — GET `https://statsapi.mlb.com/api/v1/schedule?sportId=1&season={season}&gameTypes=R&hydrate=team,linescore`
  - [ ] `Game` struct: date, home/away team id+name, home/away runs (Option), status
- [ ] In-memory cache w/ TTL (e.g., 10 min) — schedule + scores don't change often during the day. Wrap in `Mutex<HashMap<i32, CachedSeason>>` via Tauri state.

## Phase 3 — Rust backend: model

Direct port of the R math, but parameterized so the playground can call it.

- [ ] `model.rs`:
  - [ ] `TeamStats { team, runs_scored, runs_allowed, games_played, pythag_win_pct, os, ds }`
  - [ ] `compute_team_stats(games: &[Game], exponent: f64) -> (Vec<TeamStats>, f64 /* league_avg_runs */)`
  - [ ] `pythag_win_pct(rs: f64, ra: f64, exp: f64) -> f64`
  - [ ] `log5(p_a: f64, p_b: f64) -> f64`
  - [ ] `estimate_game(home: &TeamStats, away: &TeamStats, lg_avg_runs: f64) -> Prediction`
    - `Prediction { home_win_prob, away_win_prob, home_fair_odds, away_fair_odds, home_pred_runs, away_pred_runs, total_runs }`
  - [ ] `prob_to_american_odds(p: f64) -> i32`
  - [ ] `optimize_exponent(games: &[Game]) -> f64` — golden-section search on MSE between predicted and actual win% (matches R's `optimize()` behavior). Interval `[0.5, 5.0]`.

## Phase 4 — Tauri commands

Exposed to Svelte via `invoke()`:

- [ ] `get_predictions(date: Option<String>, exponent: Option<f64>) -> Vec<GameRow>` — date defaults to today; exponent defaults to the optimized value
- [ ] `get_team_stats(season: i32, exponent: f64) -> Vec<TeamStats>` — for the playground
- [ ] `get_optimal_exponent(season: i32) -> f64`
- [ ] `compute_matchup(home: TeamStats, away: TeamStats, lg_avg_runs: f64) -> Prediction` — pure-function endpoint for the playground; takes user-edited stats

## Phase 5 — Svelte frontend: Predictions tab

- [ ] Date picker (default = today)
- [ ] "Fetch predictions" button + loading spinner
- [ ] Results table: Home, Away, Win Prob (home/away), Fair Odds, Predicted Runs, Total
- [ ] Empty state ("no games scheduled") + error state (network failure)
- [ ] "Export CSV" button (reuses the same data)
- [ ] Subtle info icons on each column → tooltip explaining the metric

## Phase 6 — Svelte frontend: Learn tab

Long-form walkthrough — no API calls, pure content.

- [ ] Section 1 — What is Pythagorean Expectation? (Bill James, 1980s, intuition)
- [ ] Section 2 — The formula with KaTeX rendering:
  - $W\% = \frac{RS^x}{RS^x + RA^x}$
  - Why the exponent matters (1.83 → 2.0 in baseball; we optimize per-season)
- [ ] Section 3 — log5 (how we combine two teams' win %)
  - $P(A\text{ beats }B) = \frac{p_A(1-p_B)}{p_A(1-p_B) + (1-p_A)p_B}$
- [ ] Section 4 — Offensive/Defensive Strength → predicting runs
  - `OS = (RS/G) / lg_avg_runs`; `DS = (RA/G) / lg_avg_runs`
  - `E[home runs] = OS_home × DS_away × lg_avg_runs`
- [ ] Section 5 — From probability to fair odds (the betting-side context)
- [ ] Section 6 — Limitations (no pitcher matchups, no ballpark factors, no recency weighting)

## Phase 7 — Svelte frontend: Playground tab

This is where it earns the "learn by doing" label.

- [ ] Exponent slider (1.0 → 3.0, default = optimized value), live update
- [ ] Two `TeamCard`s with editable: Runs Scored, Runs Allowed, Games Played
  - Pre-populated with real 2025 team stats — dropdown to pick a team
- [ ] Live readout panel: Home Win %, Away Win %, Predicted Runs, Total, Fair Odds
- [ ] Small chart: Win % vs. Exponent across [1.0, 3.0] for the current matchup (so the user sees the sensitivity)
- [ ] "Reset to actuals" button

## Phase 8 — Polish + ship

- [ ] App icon (simple baseball-themed SVG → use `tauri icon` to generate platform sizes)
- [ ] Window title + sizing in `tauri.conf.json`
- [ ] Dark mode (system preference)
- [ ] `pnpm tauri build` → produces `.dmg` for macOS
- [ ] Update `README.md`: new install/run instructions, screenshots, retain credit to Bill James + baseballr (data source acknowledgment for `statsapi.mlb.com`)
- [ ] Smoke test: run from terminal, verify today's predictions match what the R script would produce for the same date

---

## Open questions / decisions to revisit

- **Cross-platform?** Mac-first, but Tauri builds for Win/Linux trivially. Plan to add a CI build matrix later, not in v1.
- **Refresh cadence?** 10-min cache feels right. Surface a "last updated" indicator.
- **Auto-update?** Skip in v1. Add Tauri updater later if you want to ship versions to users.
- **Persistence?** No DB needed — schedule fits comfortably in memory.

---

## Review

**What got built:**

- **Rust backend** (`src-tauri/src/`):
  - `mlb_api.rs` — `reqwest`-based client for `statsapi.mlb.com/api/v1/schedule`. Pulls the whole season, filters to regular season. Strongly-typed serde structs for the API response, then a flat `Game` shape we own.
  - `model.rs` — Direct port of the two R scripts: `pythag_win_pct`, `log5`, `prob_to_american_odds`, `compute_team_stats`, `estimate_game`. The `optimize()`-equivalent for the exponent is a golden-section search on MSE in `[0.5, 5.0]` (matches R's `optimize`, which uses Brent's method). 6 unit tests cover the math.
  - `lib.rs` — Tauri commands (`get_predictions`, `get_team_stats`, `get_optimal_exponent`, `refresh_schedule`, `compute_matchup`, `pythag_curve`, `american_odds`) and an `AppState` with a `Mutex<Cache>` (10-min TTL on the schedule, memoized optimal exponent per season). Lock guards are dropped before any `.await` to keep things `Send`.

- **Svelte frontend** (`src/`):
  - SvelteKit + adapter-static, Svelte 5 runes (`$state`, `$derived`).
  - `+layout.svelte`: sticky top nav, brand mark, footer credit, dark-mode via `prefers-color-scheme` in `app.css`.
  - `/` Predictions: date picker, optimized-or-manual exponent, fetch/refresh, CSV export, table with home/away win prob + fair odds + predicted runs + total. Empty state shows next scheduled date.
  - `/learn`: long-form walkthrough — history, formulas (rendered via `<Formula>` component with serif type + CSS fractions), worked example, limitations.
  - `/playground`: two editable team cards (with team-picker dropdown), exponent slider 0.5–4.0, live derived results panel, SVG sensitivity chart of home-win % vs. exponent with current-value marker. All compute is local for snappy slider drags; Rust drives the initial team-stats fetch.

- **Polish:** product name → "MLB Pythagorean", window sized 1120×780, app category set to Education, demo Tauri/Svelte/Vite logos removed, README rewritten, R scripts preserved in `legacy/` with their original README.

**Verified:**

- `cargo check` clean (0 warnings)
- `cargo build` succeeds (33MB debug binary at `src-tauri/target/debug/mlbpe`)
- `cargo test` — 6/6 passing
- `pnpm check` — 0 errors, 0 warnings across 268 files
- `pnpm build` — frontend builds cleanly to `build/`

**Not verified (needs you):**

- Live data fetch + UI flow — run `pnpm tauri dev` to see the window with today's games. The MLB Stats API call is unguarded by tests because hitting the network from CI is flaky; the structure mirrors the working R version exactly.

**Punt list (intentional):**

- Custom app icon — the generic Tauri icon is in place; `pnpm tauri icon path/to/source.png` will regenerate the full set when you have artwork.
- Cross-platform CI build matrix — Tauri supports it; not in v1.
- Auto-updater — skipped for v1.
- Park factors, starting pitchers, recency weighting — out of scope; the Learn page lists these as model limitations.

