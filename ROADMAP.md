# Roadmap

Ideas for expanding mlb-pe beyond the current Pythagorean-only model. Ordered loosely by suggested sequence — we'll tackle one at a time.

## Done

- ✅ **#1 Starting pitcher adjustment** — blends starter ERA with team RA/G (`w = 0.6`, fallback under 20 IP). Toggle to disable. Pitcher info shown on every Predictions card. Learn page section 8.
- ✅ **#2 Standings page** — six division tables + wild-card race per league using the `/standings` endpoint. Real (W-L) records now plumbed into Predictions cards.
- ✅ **#5a Home-field advantage** — log-odds shift of 0.1603 (= logit(0.54) − logit(0.50)) applied to home win probability. Shrinks at the extremes. Toggle on Predictions + Playground. Learn page section 9. Park factors (the other half of original #5) still TBD.
- ✅ **#7 Recent form weighting** — `RS/G` and `RA/G` blended 60% season + 40% L20. Aggregates completed games from the cached schedule (no extra API call). Toggle on Predictions + Playground. L20 line shown on every card. Learn page section 10.
- ✅ **Game detail view** — click any matchup card → `/game/[gamePk]`. Tier 1: a step-by-step walkthrough of how the model reached that prediction (rate chain → per-team Pythagorean → log5 → home-field → runs → odds). Tier 2: the context sections below (#8, #9, plus home/road + L10 splits). `gamePk` now threaded through the schedule — also the correct doubleheader key.
- ✅ **#8 Head-to-head history** — season series record + per-meeting list on the game detail page. Computed from the cached schedule (no API call).
- ✅ **#9 Bullpen quality** — each team's season relief line (ERA / IP / WHIP / SV) on the game detail page, via the team relief split (`sitCodes=rp`), cached 1 h. The last-3-days *fatigue* refinement (per-pitcher game logs) is still TBD.

## Remaining

- **#3 Model performance tracker**
- **#4 Live scoreboard** — *next in suggested order.*
- **#5b Park factors** — the run-multiplier half of the original #5.
- **#6 Edge / value finder**
- **#9b Bullpen fatigue** — last-3-days workload, on top of the season line already shipped.

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

## 5. Park factors (home-field shipped ✅)

The home-field half of this item is shipped (see Done section). The park-factor half is still open:

- **Park factors:** Coors Field inflates run totals; Petco suppresses them. Multiply predicted runs by the park's multi-year run multiplier (these are well-known constants, no API needed).

**Impact:** small but real. Tightens run-total accuracy by maybe 1-2%, less effect on win probability.
**Effort:** small. Constants table keyed by venue ID + a couple of lines in the runs-predicted math.

---

## 6. Edge / value finder

A view that flags games where the model is most opinionated — and games where the model disagrees most with what a 50/50 prior would say.

- Sort today's games by `|W% - 0.5|` to surface the most confident calls.
- Optional: integrate a sportsbook odds API later to compute true edge vs. market.

**Impact:** medium for users who care about picks.
**Effort:** low (sorting); larger if integrating odds.

---

## 7. Recent form weighting ✅

**Shipped.** Each team's RS/G and RA/G are blended 60% season + 40% last-20-completed-games, with a fallback to pure season when fewer than 10 completed games exist. Aggregated from the already-cached schedule (no extra API call). Predictions cards show an L20 line beneath each pitcher. Toggle on Predictions and Playground. Learn page section 10.

---

## 8. Head-to-head history ✅

**Shipped.** Season series record + a per-meeting list on the game detail page (`Rays lead the series 4-3`, with each meeting's score). Computed from completed games already in the schedule cache — no new API call.

---

## 9. Bullpen quality / fatigue ✅ (season line)

**Shipped (season line).** Each team's aggregate relief pitching for the season — ERA / IP / WHIP / SV — on the game detail page, from the team relief split (`/teams/{id}/stats?stats=statSplits&sitCodes=rp`), cached 1 h.

Still TBD: **last-3-days fatigue** (bullpen IP over the last few days). A gassed pen is a worse late-game bet than the season line suggests, but that needs per-pitcher game logs — tracked as **#9b** in Remaining.

---

## Suggested order

1. ✅ **Starting pitcher adjustment** — biggest model lift.
2. ✅ **Standings** — adds breadth, unblocks real W-L on Predictions cards.
3. ✅ **Home-field advantage** — small but real model fix.
4. ✅ **Recent form weighting** — captures hot/cold streaks the season aggregate smooths out.
5. **Live scoreboard** — *next.* Connective tissue between predictions and reality; pairs with the Predictions cards we already have. Immediate visual payoff, no new persistence layer.
6. **Model performance tracker** — proves the work is sound; valuable but has a cold-start problem (need weeks of logged predictions before the UI is interesting).
7. **Park factors** — small polish on the run-totals math; no API call needed.
8. Everything else as appetite allows.
