# CLAUDE.md

Project-specific guidance for Claude Code working on this repo. Read this first.

## What this is

A desktop app — **Rust + Tauri 2 + SvelteKit + TypeScript** — that predicts MLB games using Bill James' Pythagorean Expectation, augmented with a starting-pitcher adjustment, a home-field advantage shift, and a recent-form (L20) blend. Also has a Standings page, a Stats page (model-flavored leaderboards: luck, OS/DS, hot/cold), a Track page (historical replay vs box scores), and a Playground sandbox. It's a Tauri rewrite of the original R scripts (still in [`legacy/`](./legacy) for reference — they're not live code).

Public MLB Stats API (`https://statsapi.mlb.com`) is the only data source. No auth, no rate limits in practice. Cached in-process for 10 minutes (schedule + standings) / 1 hour (pitcher stats).

## Architecture

```
src-tauri/src/
├── mlb_api.rs     # HTTP client for /schedule, /people, /standings endpoints
├── model.rs       # Pythagorean, log5, OS/DS, pitcher blend, home-field shift,
│                  # exponent fitter
├── division_race.rs # magic number, rest-of-season expected wins
├── futures.rs     # full-slate Monte Carlo, 12-team bracket, pennant + World Series
├── backtest.rs    # replay finished games vs box scores
└── lib.rs         # Tauri commands + AppState cache (schedule + pitchers + standings)

crates/pns-core/   # vendored next-start scorer (from pitcher-next-start); frozen model in models/

src/
├── routes/
│   ├── +page.svelte                # Predictions (time-sorted slate table)
│   ├── standings/+page.svelte      # Division standings + wild-card race
│   ├── race/+page.svelte           # Futures: division, wild card, playoffs, pennant, WS
│   ├── stats/+page.svelte          # Model-flavored leaderboards (luck, OS/DS, hot/cold)
│   ├── track/+page.svelte          # Historical replay vs outcomes
│   ├── learn/+page.svelte          # Educational walkthrough w/ left TOC
│   └── playground/+page.svelte     # Team table + matchup editor
└── lib/
    ├── api.ts      # invoke() wrappers
    ├── types.ts    # TS types mirroring Rust structs
    └── format.ts   # %, odds, CSV
```

Backend → frontend bridge: Tauri's `invoke()`. Every Rust struct that crosses the bridge needs `#[serde(rename_all = "camelCase")]` because TS expects camelCase.

## The model

For each matchup:

1. Team Pythagorean W% = RS^x / (RS^x + RA^x), with `x` fit by golden-section search to minimize MSE against this season's actual W-L (typically ~1.6–1.9).
2. **Recent-form blend** (when toggle is on, team has ≥ `MIN_RECENT_GAMES = 10` completed games):
   `RS/G_eff = 0.4 · RS/G_L20 + 0.6 · RS/G_season` (same blend for RA/G).
   Computed by `compute_recent_form(&games, RECENT_FORM_WINDOW)` in `model.rs`. The recency-blended RA/G is what the pitcher adjustment then operates on — order matters.
3. **Pitcher adjustment** (when toggle is on and probable pitcher announced):
   Prefer the vendored next-start projection (`crates/pns-core`, `score_date_detailed`):
   `RA_eff/G = E[ER] + max(0, 1 − E[IP]/9) · (recency-blended) team_RA/G`.
   If no card (TBD, date ≤ artifact cutoff, no prior starts), fall back to
   `0.6 · starter_ERA + 0.4 · team_RA/G` when season IP ≥ 20; else team RA/G.
   Recopy `crates/pns-core` + `models/pns-model-v1.json` when the PNS artifact retrains.
   Score via `tokio::task::spawn_blocking` — pns-core is blocking reqwest.
4. Predicted runs = OS_eff × DS_eff × league-avg (HFA does NOT affect run totals). OS_eff uses the L20-blended RS/G, then OS and DS are pulled 40% toward 1.0 (`RATE_SHRINK`).
5. **Win probability from the run line** (live Predictions): shrink each side 40% toward league average, then P(home scores more | not tied) under a negative binomial (size 2). Replaces Pythagorean + log5. Track can turn Overdisp off to compare against log5.
6. **Home-field advantage** (when toggle is on): shift home win in *log-odds space* by `HOME_FIELD_LOG_ODDS = 0.1603` (= logit(0.54) − logit(0.50)). This bumps a 50/50 game to ~54% but shrinks at the extremes (a 90% favorite gains < 2 pts). Helper: `shift_log_odds(p, delta)` in `model.rs`.
7. Fair American odds from the home-field-adjusted win prob.

Constants live in `model.rs`: `STARTER_SHARE = 0.6`, `MIN_IP_FOR_ADJUSTMENT = 20.0`, `HOME_FIELD_LOG_ODDS = 0.1603`, `RECENT_FORM_WINDOW = 20`, `RECENT_FORM_WEIGHT = 0.4`, `MIN_RECENT_GAMES = 10`, `NB_SIZE = 2.0`, `RUN_LINE_SHRINK = 0.4`, `RATE_SHRINK = 0.4`.

All three toggles live on the Predictions page (apply server-side via `get_predictions` params `includePitchers` / `includeHomeField` / `includeRecentForm`) AND on the Playground page (apply client-side via mirrored JS math — keep the Rust and JS implementations in sync).

Futures (`get_futures`) reuses `estimate_game_with_pitchers` with pitchers forced off, then Monte Carlos the whole remaining schedule and the 12-team bracket. One sim feeds P(division), P(wild card), P(playoffs), P(pennant), and P(World Series). Ties are always awarded (head-to-head, then intradivision, then intraleague, then a seeded coin flip), so those columns sum to 1. The Playground does not mirror this — it is not a per-game formula.

## Conventions

- **Svelte 5 runes**: `$state`, `$derived.by`, `$props`, `$effect`. No `let` reactivity, no stores unless cross-route.
- **Rust serde**: snake_case in Rust, camelCase on the wire. Don't forget the `rename_all` attribute on new bridge structs.
- **CSS variables**: defined in `src/app.css`. `--ink`, `--ink-soft`, `--ink-mute`, `--bg`, `--bg-elev`, `--bg-soft`, `--line`, `--line-soft`, `--accent`, `--good`, `--bad`, `--radius`, `--radius-sm`, `--mono`, `--serif`. **Don't introduce new colors** without asking — the palette is intentional.
- **Shell width**: `.shell` (in `app.css`) is the centered container. It's wider than typical (1600px) — the user wants generous use of desktop space. Don't widen further without asking.
- **No comments unless WHY is non-obvious.** This codebase is sparsely commented on purpose.

## Gotchas (real bugs we've hit)

1. **Duplicate gamePks on the schedule.** Postponed/cancelled placeholders sit next to the rescheduled record (`detailedState` skip in `normalize`). Suspended-then-resumed games are worse: *both* listings are `Final` with the same score (e.g. 2026-06-16 Giants @ Braves, `gamePk` 824912). `dedupe_by_game_pk()` keeps one. If a keyed `{#each}` uses `gamePk` and a duplicate slips through, Svelte throws `each_key_duplicate` and the page freezes on the spinner.

2. **Doubleheader rows.** A real split-DH has two `GameRow`s with identical (home, away). In Svelte `{#each}`, **do not key by `home + away`** — it throws `each_key_duplicate` at render time, which silently aborts the table and freezes the spinner with no terminal output. Key by index, or plumb `gamePk` through. See `memory/predictions_doubleheader_key.md`.

3. **Baseball innings notation.** The API returns `inningsPitched: "35.2"` meaning 35 + 2/3 innings, **not** 35.2 decimal. `parse_innings()` in `mlb_api.rs` handles this. If you're computing IP, use that helper, or use the `outs` field (÷ 3).

4. **Date timezones.** Frontend `todayISO()` uses `new Date().toLocaleDateString("en-CA")` which yields the user's *local* date as `YYYY-MM-DD`. Backend defaults to `Utc::now()`. The two can differ by a day near midnight UTC; the frontend date picker is the source of truth for which slate the user is asking about. (Previously the frontend used UTC via `toISOString()`, which made "today" roll to tomorrow for US users in the evening — changed to local in the review pass.)

5. **Tauri 2 + Svelte 5 reactivity.** When `invoke()` returns, assign to a `$state` variable directly. Don't await inside a `$derived`.

6. **Cross-endpoint team names differ.** `/schedule` returns full names ("Tampa Bay Rays"); `/standings` returns short names ("Rays"). **Always join across endpoints by `team_id`**, never by name. The Predictions card learned this the hard way when wiring in W-L records.

7. **MLB API mixes string and int types.** `wins`/`losses`/`runDifferential` are ints, but `divisionRank`/`leagueRank`/`wildCardRank` are *strings* like `"1"`. Type DTOs accordingly and parse in normalize. If deserialization fails with "expected i64, got string," this is why.

8. **JS / Rust model drift risk.** The Playground mirrors the model in JS for instant slider feedback. When you change a constant or formula in `model.rs`, you MUST also update `src/routes/playground/+page.svelte`. Search for the constant name in both files.

9. **Track vs live Predictions.** Track replays a finished game using only `date < D` (no same-day leak). Live `get_predictions` uses the full season-to-date cache, including games already final today. Do not "fix" Track by feeding it current season totals. Live win % is Overdisp (NB size 2 + 40% run-line shrink). Track only shows toggles for wired factors.

## Commands

```bash
pnpm tauri dev                  # launch desktop dev build (Rust + Vite)
pnpm dev                        # frontend only (no Tauri), for quick CSS work
cargo check                     # quick backend type-check
cargo test --lib                # backend unit tests
cargo run --example smoke       # end-to-end: hits live MLB API, prints prediction table
cargo run --example smoke 2026 2026-05-23   # specific season + date
cargo run --example futures                 # live 30-team futures table (optional sim count)
```

The smoke test is the fastest way to validate backend changes against real data without spinning up the GUI.

## User collaboration notes

- **Don't touch what you weren't asked to.** If the user asks to change X, change X. Don't "while I'm in there" fix Y. They have been very clear about this — multiple times.
- **Don't change colors unprompted.** The palette is set.
- **Ask before opinionated UI redesigns.** Especially anything that changes layout density, color usage, or aesthetic tone.
- **Verify before claiming success.** Run `cargo check`, then the smoke test, before saying a backend change works.
- **Commits**: conventional one-line subject + body explaining the *why*, not just the *what*. Always co-author Claude.

## Roadmap

[`ROADMAP.md`](./ROADMAP.md) tracks the broader feature list. Done: **pitcher adjustment**, **standings**, **home-field advantage**, **recent-form weighting**. Remaining: live scoreboard, model performance tracker, park factors, head-to-head history, edge/value finder, bullpen quality.

## Memory

Persistent memory lives at `~/.claude/projects/-Users-adamwickwire-GitHub-mlb-pe/memory/`. The index is `MEMORY.md`. Recent entries cover the postponed-duplicates bug and the doubleheader-key bug — re-read those if working on the schedule path.
