// Division race: magic number, rest-of-season Log5 walk, Monte Carlo P(win division).
// Reuses model.rs (pythag / log5 / HFA / recent-form) — no parallel engine.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::mlb_api::{Game, GameStatus, TeamStanding};
use crate::model::{
    compute_head_to_head, compute_recent_form, compute_team_stats, estimate_game_with_pitchers,
    round_to, TeamStats, RECENT_FORM_WINDOW,
};

const DEFAULT_SEASON_GAMES: i32 = 162;
pub const DEFAULT_N_SIMS: u32 = 5000;
pub const DEFAULT_SEED: u64 = 1;

fn division_name(id: i32) -> &'static str {
    match id {
        200 => "AL West",
        201 => "AL East",
        202 => "AL Central",
        203 => "NL West",
        204 => "NL East",
        205 => "NL Central",
        _ => "Unknown division",
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WlOverride {
    pub team_id: i32,
    pub wins: i32,
    pub losses: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemainingGame {
    pub game_pk: i64,
    pub date: String,
    pub opponent: String,
    pub opponent_id: i32,
    pub home: bool,
    pub win_prob: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DivisionRaceRow {
    pub team_id: i32,
    pub team_name: String,
    pub wins: i32,
    pub losses: i32,
    pub runs_scored: i32,
    pub runs_allowed: i32,
    pub pythag_win_pct: f64,
    pub magic_number: i32,
    pub magic_vs: String,
    pub clinched: bool,
    pub eliminated: bool,
    pub games_remaining: i32,
    pub expected_remaining_wins: f64,
    pub expected_remaining_losses: f64,
    pub projected_wins: f64,
    pub projected_losses: f64,
    pub p_win_division: f64,
    pub remaining: Vec<RemainingGame>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DivisionRaceBundle {
    pub season: i32,
    pub division_id: i32,
    pub division_name: String,
    pub last_updated: String,
    pub exponent: f64,
    pub season_games: i32,
    pub n_sims: u32,
    pub seed: u64,
    pub include_home_field: bool,
    pub include_recent_form: bool,
    pub p_tie: f64,
    pub tiebreaker_note: String,
    pub teams: Vec<DivisionRaceRow>,
}

// MN = G + 1 - W - L_opp, or G - W - L_opp when this team already owns the
// completed season-series tiebreaker. Floor at 0 (already clinched vs that opponent).
pub fn magic_number(season_games: i32, wins: i32, opponent_losses: i32, owns_tiebreak: bool) -> i32 {
    let plus = if owns_tiebreak { 0 } else { 1 };
    (season_games + plus - wins - opponent_losses).max(0)
}

// Expected remaining wins is the sum of independent Log5 P(win) over remaining games.
pub fn expected_remaining_wins(win_probs: &[f64]) -> f64 {
    win_probs.iter().copied().sum()
}

fn is_remaining(g: &Game) -> bool {
    !g.is_final() && g.status != GameStatus::Other
}

// Tiny xorshift64* so sims are seedable without a rand dependency.
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: seed | 1, // never zero
        }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn gen_bool(&mut self, p: f64) -> bool {
        let p = p.clamp(0.0, 1.0);
        let u = (self.next_u64() as f64) * (1.0 / (u64::MAX as f64));
        u < p
    }
}

#[derive(Clone)]
struct SimGame {
    home_id: i32,
    away_id: i32,
    p_home: f64,
    intra: bool,
}

fn simulate_division(
    team_ids: &[i32],
    current_wins: &HashMap<i32, i32>,
    completed_h2h: &HashMap<(i32, i32), i32>,
    games: &[SimGame],
    n_sims: u32,
    seed: u64,
) -> (HashMap<i32, f64>, f64) {
    let mut unique: HashMap<i32, u32> = team_ids.iter().map(|id| (*id, 0u32)).collect();
    let mut ties = 0u32;
    let mut rng = XorShift64::new(seed);

    for _ in 0..n_sims {
        let mut wins = current_wins.clone();
        let mut h2h = completed_h2h.clone();
        for g in games {
            let home_wins = rng.gen_bool(g.p_home);
            if team_ids.contains(&g.home_id) && home_wins {
                *wins.entry(g.home_id).or_insert(0) += 1;
            }
            if team_ids.contains(&g.away_id) && !home_wins {
                *wins.entry(g.away_id).or_insert(0) += 1;
            }
            if g.intra {
                if home_wins {
                    *h2h.entry((g.home_id, g.away_id)).or_insert(0) += 1;
                } else {
                    *h2h.entry((g.away_id, g.home_id)).or_insert(0) += 1;
                }
            }
        }

        let max_w = team_ids
            .iter()
            .map(|id| *wins.get(id).unwrap_or(&0))
            .max()
            .unwrap_or(0);
        let tied: Vec<i32> = team_ids
            .iter()
            .copied()
            .filter(|id| *wins.get(id).unwrap_or(&0) == max_w)
            .collect();

        if tied.len() == 1 {
            *unique.entry(tied[0]).or_insert(0) += 1;
            continue;
        }

        // Season-series (H2H) among the tied group. Strict most H2H wins in-group
        // takes the division; anything still tied is not a unique win.
        let h2h_counts: Vec<(i32, i32)> = tied
            .iter()
            .map(|id| {
                let w: i32 = tied
                    .iter()
                    .filter(|oid| *oid != id)
                    .map(|oid| *h2h.get(&(*id, *oid)).unwrap_or(&0))
                    .sum();
                (*id, w)
            })
            .collect();
        let max_h = h2h_counts.iter().map(|(_, w)| *w).max().unwrap_or(0);
        let h_leaders: Vec<i32> = h2h_counts
            .iter()
            .filter(|(_, w)| *w == max_h)
            .map(|(id, _)| *id)
            .collect();
        if h_leaders.len() == 1 {
            *unique.entry(h_leaders[0]).or_insert(0) += 1;
        } else {
            ties += 1;
        }
    }

    let n = n_sims.max(1) as f64;
    let p: HashMap<i32, f64> = unique
        .into_iter()
        .map(|(id, c)| (id, round_to(c as f64 / n, 4)))
        .collect();
    (p, round_to(ties as f64 / n, 4))
}

fn season_length(w_l_rem: &[(i32, i32, i32)]) -> i32 {
    let scheduled = w_l_rem
        .iter()
        .map(|(w, l, r)| w + l + r)
        .max()
        .unwrap_or(DEFAULT_SEASON_GAMES);
    if scheduled <= 0 {
        DEFAULT_SEASON_GAMES
    } else {
        scheduled
    }
}

fn remaining_h2h_count(games: &[Game], a: i32, b: i32) -> i32 {
    games
        .iter()
        .filter(|g| is_remaining(g))
        .filter(|g| {
            let ids = (g.home_team_id, g.away_team_id);
            ids == (a, b) || ids == (b, a)
        })
        .count() as i32
}

fn owns_tiebreak(games: &[Game], a: i32, b: i32) -> bool {
    if remaining_h2h_count(games, a, b) > 0 {
        return false;
    }
    let h = compute_head_to_head(games, a, b);
    h.a_wins > h.b_wins
}

fn completed_h2h_map(games: &[Game], team_ids: &[i32]) -> HashMap<(i32, i32), i32> {
    let mut m = HashMap::new();
    for (i, a) in team_ids.iter().enumerate() {
        for b in team_ids.iter().skip(i + 1) {
            let h = compute_head_to_head(games, *a, *b);
            m.insert((*a, *b), h.a_wins);
            m.insert((*b, *a), h.b_wins);
        }
    }
    m
}

#[allow(clippy::too_many_arguments)]
pub fn build_division_race(
    season: i32,
    games: &[Game],
    standings: &[TeamStanding],
    division_id: i32,
    exponent: f64,
    include_home_field: bool,
    include_recent_form: bool,
    n_sims: u32,
    seed: u64,
    wl_overrides: &[WlOverride],
) -> Result<DivisionRaceBundle, String> {
    let mut div: Vec<TeamStanding> = standings
        .iter()
        .filter(|t| t.division_id == division_id)
        .cloned()
        .collect();
    if div.is_empty() {
        return Err(format!("no teams found for division {}", division_id));
    }

    let overrides: HashMap<i32, (i32, i32)> = wl_overrides
        .iter()
        .map(|o| (o.team_id, (o.wins.max(0), o.losses.max(0))))
        .collect();
    for t in &mut div {
        if let Some((w, l)) = overrides.get(&t.team_id) {
            t.wins = *w;
            t.losses = *l;
        }
    }

    let (stats, lg_avg) = compute_team_stats(games, exponent);
    let team_by_id: HashMap<i32, &TeamStats> = stats.iter().map(|t| (t.team_id, t)).collect();
    let recent_by_id = if include_recent_form {
        compute_recent_form(games, RECENT_FORM_WINDOW)
    } else {
        HashMap::new()
    };

    let div_ids: Vec<i32> = div.iter().map(|t| t.team_id).collect();
    let div_set: std::collections::HashSet<i32> = div_ids.iter().copied().collect();

    // Remaining games that involve at least one team in this division.
    let rem_games: Vec<&Game> = games
        .iter()
        .filter(|g| is_remaining(g))
        .filter(|g| div_set.contains(&g.home_team_id) || div_set.contains(&g.away_team_id))
        .collect();

    let mut remaining_by_team: HashMap<i32, Vec<RemainingGame>> =
        div_ids.iter().map(|id| (*id, Vec::new())).collect();
    let mut sim_games: Vec<SimGame> = Vec::new();

    for g in &rem_games {
        let home_stats = team_by_id.get(&g.home_team_id).copied();
        let away_stats = team_by_id.get(&g.away_team_id).copied();
        let (p_home, p_away) = match (home_stats, away_stats) {
            (Some(h), Some(a)) => {
                let hr = if include_recent_form {
                    recent_by_id.get(&g.home_team_id).copied()
                } else {
                    None
                };
                let ar = if include_recent_form {
                    recent_by_id.get(&g.away_team_id).copied()
                } else {
                    None
                };
                let pred = estimate_game_with_pitchers(
                    h,
                    a,
                    lg_avg,
                    None,
                    None,
                    hr,
                    ar,
                    exponent,
                    include_home_field,
                );
                (pred.home_win_prob, pred.away_win_prob)
            }
            _ => (0.5, 0.5),
        };

        if div_set.contains(&g.home_team_id) {
            remaining_by_team
                .entry(g.home_team_id)
                .or_default()
                .push(RemainingGame {
                    game_pk: g.game_pk,
                    date: g.date.clone(),
                    opponent: g.away_team_name.clone(),
                    opponent_id: g.away_team_id,
                    home: true,
                    win_prob: p_home,
                });
        }
        if div_set.contains(&g.away_team_id) {
            remaining_by_team
                .entry(g.away_team_id)
                .or_default()
                .push(RemainingGame {
                    game_pk: g.game_pk,
                    date: g.date.clone(),
                    opponent: g.home_team_name.clone(),
                    opponent_id: g.home_team_id,
                    home: false,
                    win_prob: p_away,
                });
        }

        sim_games.push(SimGame {
            home_id: g.home_team_id,
            away_id: g.away_team_id,
            p_home,
            intra: div_set.contains(&g.home_team_id) && div_set.contains(&g.away_team_id),
        });
    }

    for list in remaining_by_team.values_mut() {
        list.sort_by(|a, b| a.date.cmp(&b.date).then(a.game_pk.cmp(&b.game_pk)));
    }

    let rem_counts: HashMap<i32, i32> = remaining_by_team
        .iter()
        .map(|(id, v)| (*id, v.len() as i32))
        .collect();

    let w_l_rem: Vec<(i32, i32, i32)> = div
        .iter()
        .map(|t| {
            (
                t.wins,
                t.losses,
                *rem_counts.get(&t.team_id).unwrap_or(&0),
            )
        })
        .collect();
    let g_season = season_length(&w_l_rem);

    let current_wins: HashMap<i32, i32> = div.iter().map(|t| (t.team_id, t.wins)).collect();
    let h2h_map = completed_h2h_map(games, &div_ids);
    let n_sims = n_sims.clamp(100, 50_000);
    let (p_win, p_tie) = simulate_division(
        &div_ids,
        &current_wins,
        &h2h_map,
        &sim_games,
        n_sims,
        seed,
    );

    let mut rows = Vec::new();
    for t in &div {
        let rem = remaining_by_team.remove(&t.team_id).unwrap_or_default();
        let games_remaining = rem.len() as i32;
        let probs: Vec<f64> = rem.iter().map(|g| g.win_prob).collect();
        let exp_w = expected_remaining_wins(&probs);
        let exp_l = games_remaining as f64 - exp_w;

        // Binding constraint to clinch the division = the opponent with the
        // largest remaining magic number (the closest threat).
        let mut mn = 0i32;
        let mut magic_vs = String::new();
        let mut threat_wins = -1i32;
        let mut threat_losses = 999i32;
        let mut eliminated = false;
        for u in div.iter().filter(|u| u.team_id != t.team_id) {
            let owns = owns_tiebreak(games, t.team_id, u.team_id);
            let m = magic_number(g_season, t.wins, u.losses, owns);
            let u_rem = *rem_counts.get(&u.team_id).unwrap_or(&0);
            let max_units = games_remaining + u_rem;
            if m > max_units {
                eliminated = true;
            }
            if m > mn || (m == mn && (u.wins > threat_wins || (u.wins == threat_wins && u.losses < threat_losses))) {
                mn = m;
                magic_vs = u.team_name.clone();
                threat_wins = u.wins;
                threat_losses = u.losses;
            }
        }
        let clinched = mn == 0 && !eliminated;
        let stats_row = team_by_id.get(&t.team_id);

        rows.push(DivisionRaceRow {
            team_id: t.team_id,
            team_name: t.team_name.clone(),
            wins: t.wins,
            losses: t.losses,
            runs_scored: t.runs_scored,
            runs_allowed: t.runs_allowed,
            pythag_win_pct: stats_row.map(|s| round_to(s.pythag_win_pct, 4)).unwrap_or(0.5),
            magic_number: mn,
            magic_vs,
            clinched,
            eliminated,
            games_remaining,
            expected_remaining_wins: round_to(exp_w, 2),
            expected_remaining_losses: round_to(exp_l, 2),
            projected_wins: round_to(t.wins as f64 + exp_w, 2),
            projected_losses: round_to(t.losses as f64 + exp_l, 2),
            p_win_division: *p_win.get(&t.team_id).unwrap_or(&0.0),
            remaining: rem,
        });
    }

    rows.sort_by(|a, b| {
        b.wins
            .cmp(&a.wins)
            .then(a.losses.cmp(&b.losses))
            .then(a.team_name.cmp(&b.team_name))
    });

    Ok(DivisionRaceBundle {
        season,
        division_id,
        division_name: division_name(division_id).to_string(),
        last_updated: chrono::Utc::now().to_rfc3339(),
        exponent: round_to(exponent, 4),
        season_games: g_season,
        n_sims,
        seed,
        include_home_field,
        include_recent_form,
        p_tie,
        tiebreaker_note: "Magic number uses MN = G + 1 − W − L_opp (a head-to-head win is worth 2). When the season series is complete and this team leads it, the +1 drops (a tie would go to them). Incomplete series keep the standard formula — later MLB tiebreakers (intradivision record, etc.) are not applied. Simulated ties that H2H cannot break are not counted as a unique division win.".into(),
        teams: rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mlb_api::GameStatus;

    fn fin(pk: i64, date: &str, h_id: i32, h: &str, hr: i32, a_id: i32, a: &str, ar: i32) -> Game {
        Game {
            game_pk: pk,
            date: date.into(),
            status: GameStatus::Final,
            series_description: Some("Regular Season".into()),
            home_team_id: h_id,
            home_team_name: h.into(),
            home_runs: Some(hr),
            away_team_id: a_id,
            away_team_name: a.into(),
            away_runs: Some(ar),
            home_pitcher_id: None,
            home_pitcher_name: None,
            away_pitcher_id: None,
            away_pitcher_name: None,
            game_date_time: None,
            venue_id: None,
        }
    }

    fn prev(pk: i64, date: &str, h_id: i32, h: &str, a_id: i32, a: &str) -> Game {
        Game {
            game_pk: pk,
            date: date.into(),
            status: GameStatus::Preview,
            series_description: Some("Regular Season".into()),
            home_team_id: h_id,
            home_team_name: h.into(),
            home_runs: None,
            away_team_id: a_id,
            away_team_name: a.into(),
            away_runs: None,
            home_pitcher_id: None,
            home_pitcher_name: None,
            away_pitcher_id: None,
            away_pitcher_name: None,
            game_date_time: None,
            venue_id: None,
        }
    }

    fn standing(id: i32, name: &str, div: i32, w: i32, l: i32, rs: i32, ra: i32) -> TeamStanding {
        TeamStanding {
            team_id: id,
            team_name: name.into(),
            division_id: div,
            league_id: 103,
            wins: w,
            losses: l,
            pct: String::new(),
            games_back: "-".into(),
            wild_card_rank: None,
            wild_card_games_back: "-".into(),
            division_rank: None,
            league_rank: None,
            runs_scored: rs,
            runs_allowed: ra,
            run_differential: rs - ra,
            streak_code: None,
            last_ten_wins: 0,
            last_ten_losses: 0,
            division_leader: false,
            clinched: false,
        }
    }

    #[test]
    fn magic_number_classic() {
        // Leader 90-60 vs pursuer 85-65, G=162 → 162+1-90-65 = 8.
        assert_eq!(magic_number(162, 90, 65, false), 8);
        // Clinched: 100-50 vs 70-80 → 162+1-100-80 = -17 → 0.
        assert_eq!(magic_number(162, 100, 80, false), 0);
    }

    #[test]
    fn magic_number_shrinks_when_tiebreak_owned() {
        // Same records, but leader already owns the completed season series:
        // 162 - 90 - 65 = 7 (the +1 drops).
        assert_eq!(magic_number(162, 90, 65, true), 7);
        assert_eq!(magic_number(162, 90, 65, false), 8);
    }

    #[test]
    fn expected_wins_sums_log5_probs() {
        assert!((expected_remaining_wins(&[0.5, 0.5, 1.0]) - 2.0).abs() < 1e-9);
        assert!((expected_remaining_wins(&[]) - 0.0).abs() < 1e-9);
        assert!((expected_remaining_wins(&[0.25, 0.75]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn seeded_sim_certain_outcomes_are_deterministic() {
        // Two teams, 90-70 each. A has one remaining vs punching-bag C with p_home=1.
        // B has one remaining vs C with p_home=1. Both finish 91-70. No H2H → tie.
        let teams = vec![1, 2];
        let mut wins = HashMap::new();
        wins.insert(1, 90);
        wins.insert(2, 90);
        let h2h = HashMap::new();
        let games = vec![
            SimGame {
                home_id: 1,
                away_id: 99,
                p_home: 1.0,
                intra: false,
            },
            SimGame {
                home_id: 2,
                away_id: 99,
                p_home: 1.0,
                intra: false,
            },
        ];
        let (p, p_tie) = simulate_division(&teams, &wins, &h2h, &games, 200, 42);
        assert!((p_tie - 1.0).abs() < 1e-9, "expected all ties, got p_tie={p_tie}");
        assert_eq!(*p.get(&1).unwrap_or(&-1.0), 0.0);
        assert_eq!(*p.get(&2).unwrap_or(&-1.0), 0.0);
    }

    #[test]
    fn seeded_sim_h2h_tiebreak_awards_series_leader() {
        // Same as above, but team 1 already leads the season series 4-2.
        let teams = vec![1, 2];
        let mut wins = HashMap::new();
        wins.insert(1, 90);
        wins.insert(2, 90);
        let mut h2h = HashMap::new();
        h2h.insert((1, 2), 4);
        h2h.insert((2, 1), 2);
        let games = vec![
            SimGame {
                home_id: 1,
                away_id: 99,
                p_home: 1.0,
                intra: false,
            },
            SimGame {
                home_id: 2,
                away_id: 99,
                p_home: 1.0,
                intra: false,
            },
        ];
        let (p, p_tie) = simulate_division(&teams, &wins, &h2h, &games, 200, 7);
        assert!((p_tie - 0.0).abs() < 1e-9);
        assert!((p.get(&1).copied().unwrap_or(0.0) - 1.0).abs() < 1e-9);
        assert_eq!(p.get(&2).copied().unwrap_or(-1.0), 0.0);
    }

    #[test]
    fn seeded_sim_is_reproducible() {
        let teams = vec![1, 2];
        let mut wins = HashMap::new();
        wins.insert(1, 10);
        wins.insert(2, 10);
        let h2h = HashMap::new();
        let games = vec![SimGame {
            home_id: 1,
            away_id: 2,
            p_home: 0.6,
            intra: true,
        }];
        let (p1, t1) = simulate_division(&teams, &wins, &h2h, &games, 500, 12345);
        let (p2, t2) = simulate_division(&teams, &wins, &h2h, &games, 500, 12345);
        assert_eq!(p1, p2);
        assert_eq!(t1, t2);
        // A 0.6 home favorite should win the division more often than the visitor.
        assert!(p1.get(&1).copied().unwrap_or(0.0) > p1.get(&2).copied().unwrap_or(1.0));
    }

    #[test]
    fn walk_aggregates_remaining_log5() {
        // Two equal teams, each with one remaining home game against the other
        // (so one game total). Neutral site → 0.5 each. Expected remaining
        // wins for each = 0.5.
        let games = vec![
            fin(1, "2026-04-01", 10, "A", 5, 20, "B", 5),
            fin(2, "2026-04-02", 20, "B", 5, 10, "A", 5),
            prev(10, "2026-09-01", 10, "A", 20, "B"),
        ];
        let standings = vec![
            standing(10, "A", 201, 1, 1, 10, 10),
            standing(20, "B", 201, 1, 1, 10, 10),
        ];
        let bundle = build_division_race(
            2026, &games, &standings, 201, 2.0, false, false, 200, 1, &[],
        )
        .expect("build");
        assert_eq!(bundle.teams.len(), 2);
        for row in &bundle.teams {
            assert_eq!(row.games_remaining, 1);
            assert!((row.expected_remaining_wins - 0.5).abs() < 0.08);
            assert_eq!(row.remaining.len(), 1);
        }
        // Neutral 0.5 game + equal records → unique win only via that game, no leftover tie.
        let p_sum: f64 = bundle.teams.iter().map(|t| t.p_win_division).sum();
        assert!((p_sum + bundle.p_tie - 1.0).abs() < 1e-6);
    }

    #[test]
    fn magic_number_uses_closest_threat() {
        let games = vec![
            fin(1, "2026-04-01", 1, "Lead", 6, 2, "Second", 3),
            fin(2, "2026-04-02", 1, "Lead", 5, 3, "Last", 1),
            prev(10, "2026-09-01", 1, "Lead", 99, "Out"),
            prev(11, "2026-09-02", 2, "Second", 99, "Out"),
            prev(12, "2026-09-03", 3, "Last", 99, "Out"),
        ];
        // Need TeamStats for opponent 99 as well — give them a final so they exist.
        let games = {
            let mut g = games;
            g.push(fin(3, "2026-04-03", 99, "Out", 4, 1, "Lead", 4));
            g
        };
        let standings = vec![
            standing(1, "Lead", 201, 90, 60, 500, 400),
            standing(2, "Second", 201, 85, 65, 450, 450),
            standing(3, "Last", 201, 70, 80, 400, 500),
        ];
        let bundle = build_division_race(
            2026, &games, &standings, 201, 2.0, false, false, 200, 1, &[],
        )
        .expect("build");
        let lead = bundle.teams.iter().find(|t| t.team_id == 1).unwrap();
        // G = max(90+60+1, 85+65+1, 70+80+1) = 151. MN vs Second = 151+1-90-65 = -3 → 0?
        // 90+60+1=151, yes. That would clinch already with G=151.
        // The test is really: magic_vs is Second (closest threat), not Last.
        assert_eq!(lead.magic_vs, "Second");
    }
}
