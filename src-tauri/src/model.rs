// Pythagorean Expectation + log5 + run-environment model.
// Direct port of the two R scripts in legacy/, parameterized so callers can sweep the exponent.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::mlb_api::Game;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStats {
    pub team_id: i32,
    pub team: String,
    pub runs_scored: i32,
    pub runs_allowed: i32,
    pub games_played: i32,
    pub pythag_win_pct: f64,
    pub os: f64,
    pub ds: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    pub home_win_prob: f64,
    pub away_win_prob: f64,
    pub home_fair_odds: i32,
    pub away_fair_odds: i32,
    pub home_pred_runs: f64,
    pub away_pred_runs: f64,
    pub total_runs: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitcherInfo {
    pub name: String,
    pub era: f64,
    pub innings_pitched: f64,
    pub games_started: i32,
    // True when this pitcher's stats actually shifted the prediction.
    // False if the user turned the toggle off OR the sample was too small.
    pub applied: bool,
    // True when the pitcher's IP meets MIN_IP_FOR_ADJUSTMENT. Lets the frontend
    // show "(small sample)" without re-deriving the threshold in JS.
    pub eligible_sample: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameRow {
    pub date: String,
    pub home: String,
    pub away: String,
    #[serde(rename = "homePitcher")]
    pub home_pitcher: Option<PitcherInfo>,
    #[serde(rename = "awayPitcher")]
    pub away_pitcher: Option<PitcherInfo>,
    #[serde(flatten)]
    pub pred: Prediction,
}

// Bill James' Pythagorean win expectancy.
//   W% = RS^x / (RS^x + RA^x)
pub fn pythag_win_pct(rs: f64, ra: f64, exponent: f64) -> f64 {
    let num = rs.powf(exponent);
    let denom = num + ra.powf(exponent);
    if denom == 0.0 { 0.5 } else { num / denom }
}

// log5: probability team A beats team B given each team's standalone win %.
pub fn log5(p_a: f64, p_b: f64) -> f64 {
    let num = p_a * (1.0 - p_b);
    let denom = num + (1.0 - p_a) * p_b;
    if denom == 0.0 { 0.5 } else { num / denom }
}

pub fn prob_to_american_odds(p: f64) -> i32 {
    if !(0.0..1.0).contains(&p) || p == 0.0 {
        return 0;
    }
    if p > 0.5 {
        (-100.0 * p / (1.0 - p)).round() as i32
    } else {
        ((1.0 - p) * 100.0 / p).round() as i32
    }
}

// Aggregate season-to-date team stats from completed games.
// Returns (team stats, league-average runs per team per game).
pub fn compute_team_stats(games: &[Game], exponent: f64) -> (Vec<TeamStats>, f64) {
    let mut agg: HashMap<i32, AggRow> = HashMap::new();
    let mut total_runs: i64 = 0;
    let mut total_finished: i64 = 0;

    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap();
        let ar = g.away_runs.unwrap();
        total_runs += (hr + ar) as i64;
        total_finished += 1;

        let h = agg
            .entry(g.home_team_id)
            .or_insert_with(|| AggRow::new(g.home_team_name.clone()));
        h.runs_scored += hr as i64;
        h.runs_allowed += ar as i64;
        h.games_played += 1;

        let a = agg
            .entry(g.away_team_id)
            .or_insert_with(|| AggRow::new(g.away_team_name.clone()));
        a.runs_scored += ar as i64;
        a.runs_allowed += hr as i64;
        a.games_played += 1;
    }

    let lg_avg_runs = if total_finished > 0 {
        total_runs as f64 / (2.0 * total_finished as f64)
    } else {
        4.5
    };

    let mut stats: Vec<TeamStats> = agg
        .into_iter()
        .filter(|(_, row)| row.games_played > 0)
        .map(|(team_id, row)| {
            let rs = row.runs_scored as f64;
            let ra = row.runs_allowed as f64;
            let gp = row.games_played as f64;
            TeamStats {
                team_id,
                team: row.name,
                runs_scored: row.runs_scored as i32,
                runs_allowed: row.runs_allowed as i32,
                games_played: row.games_played as i32,
                pythag_win_pct: pythag_win_pct(rs, ra, exponent),
                os: (rs / gp) / lg_avg_runs,
                ds: (ra / gp) / lg_avg_runs,
            }
        })
        .collect();

    stats.sort_by(|a, b| a.team.cmp(&b.team));
    (stats, lg_avg_runs)
}

pub fn estimate_game(home: &TeamStats, away: &TeamStats, lg_avg_runs: f64) -> Prediction {
    estimate_game_with_pitchers(home, away, lg_avg_runs, None, None, 2.0)
}

// Per Bill James / sabermetrics consensus: a starting pitcher is responsible for ~5.4
// of 9 innings on average (~0.6 of the game's runs allowed). The remainder is the
// bullpen, which is implicitly captured in the team's season RA/G.
pub const STARTER_SHARE: f64 = 0.6;
// Below this, the pitcher sample is too small to trust — fall back to team RA.
pub const MIN_IP_FOR_ADJUSTMENT: f64 = 20.0;

#[derive(Debug, Clone, Copy)]
pub struct PitcherAdj {
    pub era: f64,
    pub innings_pitched: f64,
}

// Blend the team's season RA/G with the starter's ERA. The starter accounts for
// STARTER_SHARE of the runs allowed; the remainder is the team's season-average
// defense (which already includes bullpen + average starter pool).
//
// If the pitcher's sample is below MIN_IP_FOR_ADJUSTMENT we fall back to the team
// RA — a tiny sample with an extreme ERA shouldn't crater a prediction.
fn effective_ra_per_game(team: &TeamStats, pitcher: Option<PitcherAdj>) -> f64 {
    let team_ra_pg = if team.games_played > 0 {
        team.runs_allowed as f64 / team.games_played as f64
    } else {
        4.5
    };
    match pitcher {
        Some(p) if p.innings_pitched >= MIN_IP_FOR_ADJUSTMENT => {
            STARTER_SHARE * p.era + (1.0 - STARTER_SHARE) * team_ra_pg
        }
        _ => team_ra_pg,
    }
}

pub fn estimate_game_with_pitchers(
    home: &TeamStats,
    away: &TeamStats,
    lg_avg_runs: f64,
    home_pitcher: Option<PitcherAdj>,
    away_pitcher: Option<PitcherAdj>,
    exponent: f64,
) -> Prediction {
    // Team RS/G is fixed (offense doesn't change night-to-night), but RA/G shifts
    // toward the starting pitcher's ERA.
    let home_rs_pg = if home.games_played > 0 {
        home.runs_scored as f64 / home.games_played as f64
    } else {
        4.5
    };
    let away_rs_pg = if away.games_played > 0 {
        away.runs_scored as f64 / away.games_played as f64
    } else {
        4.5
    };
    let home_ra_eff = effective_ra_per_game(home, home_pitcher);
    let away_ra_eff = effective_ra_per_game(away, away_pitcher);

    // Recompute team Pythagorean win % using the matchup-specific RA.
    let home_pyt = pythag_win_pct(home_rs_pg, home_ra_eff, exponent);
    let away_pyt = pythag_win_pct(away_rs_pg, away_ra_eff, exponent);

    // Predicted runs: keep OS unchanged (offense is offense), but recompute DS
    // from the effective RA so an ace suppresses opposing runs.
    let home_ds_eff = home_ra_eff / lg_avg_runs;
    let away_ds_eff = away_ra_eff / lg_avg_runs;
    let home_pred = home.os * away_ds_eff * lg_avg_runs;
    let away_pred = away.os * home_ds_eff * lg_avg_runs;
    let total = home_pred + away_pred;

    let home_win = log5(home_pyt, away_pyt);
    let away_win = 1.0 - home_win;
    Prediction {
        home_win_prob: round_to(home_win, 4),
        away_win_prob: round_to(away_win, 4),
        home_fair_odds: prob_to_american_odds(home_win),
        away_fair_odds: prob_to_american_odds(away_win),
        home_pred_runs: round_to(home_pred, 2),
        away_pred_runs: round_to(away_pred, 2),
        total_runs: round_to(total, 2),
    }
}

// Find the exponent (in [0.5, 5.0]) that minimizes MSE between predicted and actual win%
// across all teams. Golden-section search — R's `optimize()` does the same in spirit (Brent's
// method, which is golden-section + parabolic interpolation).
pub fn optimize_exponent(games: &[Game]) -> f64 {
    let mut agg: HashMap<i32, AggRow> = HashMap::new();
    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap();
        let ar = g.away_runs.unwrap();
        let home_won = (hr > ar) as i64;
        let away_won = (ar > hr) as i64;

        let h = agg
            .entry(g.home_team_id)
            .or_insert_with(|| AggRow::new(g.home_team_name.clone()));
        h.runs_scored += hr as i64;
        h.runs_allowed += ar as i64;
        h.wins += home_won;
        h.games_played += 1;

        let a = agg
            .entry(g.away_team_id)
            .or_insert_with(|| AggRow::new(g.away_team_name.clone()));
        a.runs_scored += ar as i64;
        a.runs_allowed += hr as i64;
        a.wins += away_won;
        a.games_played += 1;
    }

    let teams: Vec<(f64, f64, f64)> = agg
        .values()
        .filter(|r| r.games_played > 0)
        .map(|r| {
            (
                r.runs_scored as f64,
                r.runs_allowed as f64,
                r.wins as f64 / r.games_played as f64,
            )
        })
        .collect();

    if teams.len() < 2 {
        return 2.0;
    }

    let mse = |exp: f64| -> f64 {
        let mut s = 0.0;
        for (rs, ra, actual) in &teams {
            let pred = pythag_win_pct(*rs, *ra, exp);
            s += (actual - pred).powi(2);
        }
        s / teams.len() as f64
    };

    golden_section(0.5, 5.0, 1e-4, mse)
}

fn golden_section<F: Fn(f64) -> f64>(mut lo: f64, mut hi: f64, tol: f64, f: F) -> f64 {
    // phi = (sqrt(5) - 1) / 2 ≈ 0.618
    let phi = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut c = hi - phi * (hi - lo);
    let mut d = lo + phi * (hi - lo);
    let mut fc = f(c);
    let mut fd = f(d);
    while (hi - lo).abs() > tol {
        if fc < fd {
            hi = d;
            d = c;
            fd = fc;
            c = hi - phi * (hi - lo);
            fc = f(c);
        } else {
            lo = c;
            c = d;
            fc = fd;
            d = lo + phi * (hi - lo);
            fd = f(d);
        }
    }
    round_to((lo + hi) / 2.0, 4)
}

pub fn round_to(x: f64, places: u32) -> f64 {
    let m = 10f64.powi(places as i32);
    (x * m).round() / m
}

struct AggRow {
    name: String,
    runs_scored: i64,
    runs_allowed: i64,
    wins: i64,
    games_played: i64,
}

impl AggRow {
    fn new(name: String) -> Self {
        Self {
            name,
            runs_scored: 0,
            runs_allowed: 0,
            wins: 0,
            games_played: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pythag_classic_exponent() {
        // A team that scores and allows the same number of runs should be 50/50.
        assert!((pythag_win_pct(700.0, 700.0, 2.0) - 0.5).abs() < 1e-9);
        // Bigger differential → bigger win %.
        assert!(pythag_win_pct(800.0, 600.0, 2.0) > 0.6);
    }

    #[test]
    fn log5_equal_teams() {
        assert!((log5(0.5, 0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn log5_strong_vs_weak() {
        // A .700 team vs a .300 team in log5 → much greater than 0.5
        assert!(log5(0.7, 0.3) > 0.8);
    }

    #[test]
    fn prob_to_odds_pick_em() {
        assert_eq!(prob_to_american_odds(0.5), 100);
    }

    #[test]
    fn prob_to_odds_favorite() {
        // -150 → implied probability 60%, so 0.6 → -150
        assert_eq!(prob_to_american_odds(0.6), -150);
    }

    #[test]
    fn prob_to_odds_dog() {
        // +150 → implied probability 40%
        assert_eq!(prob_to_american_odds(0.4), 150);
    }
}
