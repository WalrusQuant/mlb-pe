# CLAUDE.md

Project-specific guidance for Claude Code working on this repo. Read this first.

## What this is

A desktop app — **Rust + Tauri 2 + SvelteKit + TypeScript** — that predicts MLB games using Bill James' Pythagorean Expectation, augmented with a starting-pitcher adjustment. It's a Tauri rewrite of the original R scripts (still in [`legacy/`](./legacy) for reference — they're not live code).

Public MLB Stats API (`https://statsapi.mlb.com`) is the only data source. No auth, no rate limits in practice. Cached in-process for 10 minutes (schedule) / 1 hour (pitcher stats).

## Architecture

```
src-tauri/src/
├── mlb_api.rs     # HTTP client for /schedule and /people endpoints
├── model.rs       # Pythagorean, log5, OS/DS, pitcher blend, exponent fitter
└── lib.rs         # Tauri commands + AppState cache

src/
├── routes/
│   ├── +page.svelte                # Predictions (card-per-matchup)
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
2. **Pitcher adjustment** (when toggle is on and probable pitcher announced):
   `effective_RA/G = 0.6 · starter_ERA + 0.4 · team_RA/G`
   Below `MIN_IP = 20` innings, fall back to pure team RA.
3. Matchup win prob via log5.
4. Predicted runs = OS × DS_effective × league-avg.
5. Fair American odds from the win prob.

Constants live in `model.rs`: `STARTER_SHARE = 0.6`, `MIN_IP_FOR_ADJUSTMENT = 20.0`.

## Conventions

- **Svelte 5 runes**: `$state`, `$derived.by`, `$props`, `$effect`. No `let` reactivity, no stores unless cross-route.
- **Rust serde**: snake_case in Rust, camelCase on the wire. Don't forget the `rename_all` attribute on new bridge structs.
- **CSS variables**: defined in `src/app.css`. `--ink`, `--ink-soft`, `--ink-mute`, `--bg`, `--bg-elev`, `--bg-soft`, `--line`, `--line-soft`, `--accent`, `--good`, `--bad`, `--radius`, `--radius-sm`, `--mono`, `--serif`. **Don't introduce new colors** without asking — the palette is intentional.
- **Shell width**: `.shell` (in `app.css`) is the centered container. It's wider than typical (1600px) — the user wants generous use of desktop space. Don't widen further without asking.
- **No comments unless WHY is non-obvious.** This codebase is sparsely commented on purpose.

## Gotchas (real bugs we've hit)

1. **Postponed-game duplicates.** The MLB schedule endpoint returns *two* records for any postponed-then-rescheduled game (same gamePk, different `dates[]` entries). One has `detailedState = "Postponed"` with `officialDate` pointing at the rescheduled day. `normalize()` in `mlb_api.rs` skips `detailedState in {Postponed, Cancelled}` to dedupe. If you ever see a "triple-header," this is back.

2. **Doubleheader rows.** A real split-DH has two `GameRow`s with identical (home, away). In Svelte `{#each}`, **do not key by `home + away`** — it throws `each_key_duplicate` at render time, which silently aborts the table and freezes the spinner with no terminal output. Key by index, or plumb `gamePk` through. See `memory/predictions_doubleheader_key.md`.

3. **Baseball innings notation.** The API returns `inningsPitched: "35.2"` meaning 35 + 2/3 innings, **not** 35.2 decimal. `parse_innings()` in `mlb_api.rs` handles this. If you're computing IP, use that helper, or use the `outs` field (÷ 3).

4. **Date timezones.** Frontend uses `new Date().toISOString().slice(0, 10)` which is UTC. Backend defaults to `Utc::now()`. After ~7 PM CDT this means "today" rolls to tomorrow's date — that's intentional, matches the API's `officialDate`.

5. **Tauri 2 + Svelte 5 reactivity.** When `invoke()` returns, assign to a `$state` variable directly. Don't await inside a `$derived`.

## Commands

```bash
pnpm tauri dev                  # launch desktop dev build (Rust + Vite)
pnpm dev                        # frontend only (no Tauri), for quick CSS work
cargo check                     # quick backend type-check
cargo test --lib                # backend unit tests
cargo run --example smoke       # end-to-end: hits live MLB API, prints prediction table
cargo run --example smoke 2026 2026-05-23   # specific season + date
```

The smoke test is the fastest way to validate backend changes against real data without spinning up the GUI.

## User collaboration notes

- **Don't touch what you weren't asked to.** If the user asks to change X, change X. Don't "while I'm in there" fix Y. They have been very clear about this — multiple times.
- **Don't change colors unprompted.** The palette is set.
- **Ask before opinionated UI redesigns.** Especially anything that changes layout density, color usage, or aesthetic tone.
- **Verify before claiming success.** Run `cargo check`, then the smoke test, before saying a backend change works.
- **Commits**: conventional one-line subject + body explaining the *why*, not just the *what*. Always co-author Claude.

## Roadmap

[`ROADMAP.md`](./ROADMAP.md) tracks the broader feature list. Pitcher adjustment is the first item — done. Standings, model performance tracker, live scoreboard, park factors, and a few smaller ideas remain.

## Memory

Persistent memory lives at `~/.claude/projects/-Users-adamwickwire-GitHub-mlb-pe/memory/`. The index is `MEMORY.md`. Recent entries cover the postponed-duplicates bug and the doubleheader-key bug — re-read those if working on the schedule path.
