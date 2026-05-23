# Roadmap

Ideas for expanding mlb-pe beyond the current Pythagorean-only model. Ordered loosely by suggested sequence — we'll tackle one at a time.

## Done

- ✅ **#1 Starting pitcher adjustment** — blends starter ERA with team RA/G (`w = 0.6`, fallback under 20 IP). Toggle to disable. Pitcher info shown on every Predictions card. Learn page section 8.
- ✅ **#2 Standings page** — six division tables + wild-card race per league using the `/standings` endpoint. Real (W-L) records now plumbed into Predictions cards.

## Remaining

---

## 1. Starting pitcher adjustment ✅

**Shipped.** Originally — the single biggest blind spot in the base Pythagorean model. Pythagorean knows team-level run scoring and run prevention, but had no idea whether tonight's starter is the ace or a spot starter.

- Pull `probablePitcher` per game from the schedule endpoint (`/schedule?hydrate=probablePitcher`).
- Pull each pitcher's season ERA / FIP / IP from the people endpoint.
- Shift each team's effective run-allowed rate toward the starter's ERA (weighted by typical starter IP ≈ 60% of a game).
- Recompute Pythagorean W% with the adjusted RA before feeding into log5.
- Surface the pitcher matchup on the Predictions card (e.g. `Skubal (2.41) vs. Civale (4.83)`).

**Impact:** large. Pitcher quality is usually a bigger per-game factor than team-level differentials.
**Effort:** medium. Adds an API call, a small new struct, and a model-tuning constant.

---

## 2. Standings page ✅

**Shipped.** Six division tables (AL East / Central / West, NL East / Central / West) plus a wild-card race section per league via `/standings?leagueId=103,104`. Shows W · L · PCT · GB · L10 · Streak · Run Diff per row; division leaders flagged; teams in WC position tinted. Real W-L records joined into the Predictions cards by team_id.

Not yet wired: clicking a team to jump to a per-team detail view. Could revisit when there's a use for it.

---

## 3. Model performance tracker

> Don't ask people to trust the model. Show them whether it's actually right.

- Log every prediction to a small SQLite file on disk (date, matchup, predicted W%, predicted total, exponent used).
- When a game finishes, join with actuals and store the result.
- New nav tab: **Track Record**.
  - Headline numbers: hit rate (favorite-wins), Brier score, log loss.
  - Calibration plot: predicted W% bucket vs. actual win rate. (When we say 60%, do teams actually win 60%?)
  - Run-total error histogram.
  - Cumulative profit if you bet every fair-odds edge ≥ X%.

**Impact:** large for credibility. Also catches model regressions early.
**Effort:** medium. New SQLite layer + a backfill job for past predictions.

---

## 4. Live scoreboard

Today's games with running scores, current inning, base/out state.

- Use `/schedule?hydrate=linescore,probablePitcher,decisions`.
- Add a strip at the top of the Predictions page showing each game's live state next to its prediction: `ATL 4 - 3 WSH · T7 · 1 out · 2-1 count`.
- Highlight games where our model strongly favored a team that's currently losing (or vice versa) — the "model is being challenged" frame.
- Refresh on a short interval while there are live games.

**Impact:** medium. High emotional payoff — you can watch the model be right or wrong in real time.
**Effort:** medium. New polling logic, careful not to spam the API.

---

## 5. Park factors + home-field edge

Two small constants that fix two real model blind spots.

- **Park factors:** Coors Field inflates run totals; Petco suppresses them. Multiply predicted runs by the park's multi-year run multiplier (these are well-known constants, no API needed).
- **Home-field edge:** MLB home teams win ~54% historically. Apply a small bump to home W% post-log5.

**Impact:** small but real. Tightens prediction accuracy by maybe 1-2%.
**Effort:** small. Constants + a couple of lines in the model.

---

## 6. Edge / value finder

A view that flags games where the model is most opinionated — and games where the model disagrees most with what a 50/50 prior would say.

- Sort today's games by `|W% - 0.5|` to surface the most confident calls.
- Optional: integrate a sportsbook odds API later to compute true edge vs. market.

**Impact:** medium for users who care about picks.
**Effort:** low (sorting); larger if integrating odds.

---

## 7. Recent form weighting

Pythagorean weights April and September equally. A team's last 20 games is often more predictive than their full-season number.

- Compute a `pythag_l20` alongside `pythag_full`.
- Blend them (e.g. 60% full season, 40% L20) for the prediction.
- Expose both in the Playground so the user can see the difference.

**Impact:** small-to-medium. Helps with hot/cold streaks; hurts when teams are in genuine transition (injuries, trades).
**Effort:** medium. Needs game-by-game team aggregation, not just season totals.

---

## 8. Head-to-head history

For each predicted matchup, show season series record (`ATL leads season series 4-2`).
Pull from completed games already in our schedule cache. No new API call.

**Impact:** small. Nice color, not a model improvement.
**Effort:** trivial.

---

## 9. Bullpen quality / fatigue

Bullpen ERA and innings pitched over the last 3 days.
A team with a gassed pen is a worse late-game bet than their season RA suggests.

**Impact:** small-to-medium.
**Effort:** medium. Requires per-pitcher game logs.

---

## Suggested order

1. ✅ **Starting pitcher adjustment** — biggest model lift.
2. ✅ **Standings** — adds breadth, unblocks real W-L on Predictions cards.
3. **Live scoreboard** — *next.* Connective tissue between predictions and reality; pairs with the Predictions cards we already have. Immediate visual payoff, no new persistence layer.
4. **Model performance tracker** — proves the work is sound; valuable but has a cold-start problem (need weeks of logged predictions before the UI is interesting).
5. **Park factors + home-field edge** — small polish on the model.
6. Everything else as appetite allows.
