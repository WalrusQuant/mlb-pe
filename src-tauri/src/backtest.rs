// Replay completed games: predict from everything known *before* that date,
// then compare to the box score. Not a live tracker.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;

use crate::mlb_api::Game;
use crate::model::{
    compute_recent_form, compute_team_stats, estimate_game_with_pitchers, optimize_exponent,
    round_to, PitcherAdj, MIN_IP_FOR_ADJUSTMENT, MIN_RECENT_GAMES, RECENT_FORM_WINDOW,
};

const MIN_PRIOR_GAMES: i32 = 10;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestGame {
    pub game_pk: i64,
    pub date: String,
    pub home: String,
    pub away: String,
    pub p_home: f64,
    pub home_won: bool,
    pub picked_home: bool,
    pub hit: bool,
    pub pred_home_runs: f64,
    pub pred_away_runs: f64,
    pub actual_home_runs: i32,
    pub actual_away_runs: i32,
    pub brier: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibBucket {
    pub label: String,
    pub predicted: f64,
    pub actual: f64,
    pub n: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthRow {
    pub month: String,
    pub n: u32,
    pub hit_rate: f64,
    pub brier: f64,
    pub total_runs_mae: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestBundle {
    pub season: i32,
    pub n: u32,
    pub skipped_early: u32,
    pub skipped_tied: u32,
    pub hit_rate: f64,
    pub brier: f64,
    pub log_loss: f64,
    pub total_runs_mae: f64,
    pub home_runs_mae: f64,
    pub away_runs_mae: f64,
    pub include_pitchers: bool,
    pub include_home_field: bool,
    pub include_recent_form: bool,
    pub calibration: Vec<CalibBucket>,
    pub monthly: Vec<MonthRow>,
    pub games: Vec<BacktestGame>,
}

#[derive(Debug, Clone)]
pub struct StarterLog {
    pub pitcher_id: i32,
    pub game_pk: i64,
    pub game_date: NaiveDate,
    pub home: bool,
    pub innings_pitched: f64,
    pub earned_runs: f64,
}

pub fn run_backtest(
    season: i32,
    games: &[Game],
    logs: &[StarterLog],
    include_pitchers: bool,
    include_home_field: bool,
    include_recent_form: bool,
) -> BacktestBundle {
    let games = crate::mlb_api::dedupe_by_game_pk(games.to_vec());
    let games = &games;
    let mut by_date: Vec<String> = games
        .iter()
        .filter(|g| g.is_final())
        .map(|g| g.date.clone())
        .collect();
    by_date.sort();
    by_date.dedup();

    let mut rows: Vec<BacktestGame> = Vec::new();
    let mut skipped_early = 0u32;
    let mut skipped_tied = 0u32;
    let mut prior: Vec<Game> = Vec::new();

    for date in &by_date {
        let todays: Vec<&Game> = games
            .iter()
            .filter(|g| g.is_final() && g.date == *date)
            .collect();
        if todays.is_empty() {
            continue;
        }

        let exp = optimize_exponent(&prior);
        let (stats, lg_avg) = compute_team_stats(&prior, exp);
        let by_id: HashMap<i32, &crate::model::TeamStats> =
            stats.iter().map(|t| (t.team_id, t)).collect();
        let recent = if include_recent_form {
            compute_recent_form(&prior, RECENT_FORM_WINDOW)
        } else {
            HashMap::new()
        };
        let as_of = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok();

        for g in &todays {
            let Some(home) = by_id.get(&g.home_team_id) else {
                skipped_early += 1;
                continue;
            };
            let Some(away) = by_id.get(&g.away_team_id) else {
                skipped_early += 1;
                continue;
            };
            if home.games_played < MIN_PRIOR_GAMES || away.games_played < MIN_PRIOR_GAMES {
                skipped_early += 1;
                continue;
            }
            let hr = g.home_runs.unwrap_or(0);
            let ar = g.away_runs.unwrap_or(0);
            if hr == ar {
                skipped_tied += 1;
                continue;
            }

            let home_p = if include_pitchers {
                pitcher_for(g, true, as_of, logs)
            } else {
                None
            };
            let away_p = if include_pitchers {
                pitcher_for(g, false, as_of, logs)
            } else {
                None
            };
            let home_r = if include_recent_form {
                recent.get(&g.home_team_id).copied()
            } else {
                None
            };
            let away_r = if include_recent_form {
                recent.get(&g.away_team_id).copied()
            } else {
                None
            };

            let pred = estimate_game_with_pitchers(
                home,
                away,
                lg_avg,
                home_p,
                away_p,
                home_r.filter(|r| r.games >= MIN_RECENT_GAMES),
                away_r.filter(|r| r.games >= MIN_RECENT_GAMES),
                exp,
                include_home_field,
            );

            let p = pred.home_win_prob.clamp(1e-6, 1.0 - 1e-6);
            let home_won = hr > ar;
            let picked_home = p >= 0.5;
            let y = if home_won { 1.0 } else { 0.0 };
            let brier = (p - y).powi(2);

            rows.push(BacktestGame {
                game_pk: g.game_pk,
                date: g.date.clone(),
                home: g.home_team_name.clone(),
                away: g.away_team_name.clone(),
                p_home: round_to(p, 4),
                home_won,
                picked_home,
                hit: picked_home == home_won,
                pred_home_runs: pred.home_pred_runs,
                pred_away_runs: pred.away_pred_runs,
                actual_home_runs: hr,
                actual_away_runs: ar,
                brier: round_to(brier, 4),
            });
        }

        prior.extend(todays.into_iter().map(|g| g.clone()));
    }

    summarize(
        season,
        include_pitchers,
        include_home_field,
        include_recent_form,
        skipped_early,
        skipped_tied,
        rows,
    )
}

fn pitcher_for(
    g: &Game,
    home: bool,
    as_of: Option<NaiveDate>,
    logs: &[StarterLog],
) -> Option<PitcherAdj> {
    let as_of = as_of?;
    let pk = g.game_pk;
    let starter = logs.iter().find(|l| l.game_pk == pk && l.home == home)?;
    era_as_of(logs, starter.pitcher_id, as_of)
        .map(|(era, ip)| PitcherAdj::from_era(era, ip))
}

fn era_as_of(logs: &[StarterLog], pitcher_id: i32, before: NaiveDate) -> Option<(f64, f64)> {
    let mut ip = 0.0;
    let mut er = 0.0;
    for l in logs {
        if l.pitcher_id == pitcher_id && l.game_date < before {
            ip += l.innings_pitched;
            er += l.earned_runs;
        }
    }
    if ip < MIN_IP_FOR_ADJUSTMENT {
        None
    } else {
        Some((9.0 * er / ip, ip))
    }
}

fn summarize(
    season: i32,
    include_pitchers: bool,
    include_home_field: bool,
    include_recent_form: bool,
    skipped_early: u32,
    skipped_tied: u32,
    mut rows: Vec<BacktestGame>,
) -> BacktestBundle {
    let n = rows.len() as f64;
    let (hit, brier, ll, tmae, hmae, amae) = if n == 0.0 {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    } else {
        let hit = rows.iter().filter(|r| r.hit).count() as f64 / n;
        let brier = rows.iter().map(|r| r.brier).sum::<f64>() / n;
        let ll = rows
            .iter()
            .map(|r| {
                let p = r.p_home;
                let y = if r.home_won { 1.0 } else { 0.0 };
                -(y * p.ln() + (1.0 - y) * (1.0 - p).ln())
            })
            .sum::<f64>()
            / n;
        let tmae = rows
            .iter()
            .map(|r| {
                ((r.pred_home_runs + r.pred_away_runs)
                    - (r.actual_home_runs + r.actual_away_runs) as f64)
                    .abs()
            })
            .sum::<f64>()
            / n;
        let hmae = rows
            .iter()
            .map(|r| (r.pred_home_runs - r.actual_home_runs as f64).abs())
            .sum::<f64>()
            / n;
        let amae = rows
            .iter()
            .map(|r| (r.pred_away_runs - r.actual_away_runs as f64).abs())
            .sum::<f64>()
            / n;
        (hit, brier, ll, tmae, hmae, amae)
    };

    let calibration = calibration(&rows);
    let monthly = monthly(&rows);
    rows.sort_by(|a, b| b.date.cmp(&a.date).then(a.game_pk.cmp(&b.game_pk)));

    BacktestBundle {
        season,
        n: n as u32,
        skipped_early,
        skipped_tied,
        hit_rate: round_to(hit, 4),
        brier: round_to(brier, 4),
        log_loss: round_to(ll, 4),
        total_runs_mae: round_to(tmae, 3),
        home_runs_mae: round_to(hmae, 3),
        away_runs_mae: round_to(amae, 3),
        include_pitchers,
        include_home_field,
        include_recent_form,
        calibration,
        monthly,
        games: rows,
    }
}

fn calibration(rows: &[BacktestGame]) -> Vec<CalibBucket> {
    let mut buckets: Vec<(f64, f64, u32)> = (0..10).map(|_| (0.0, 0.0, 0)).collect();
    for r in rows {
        let fav = if r.p_home >= 0.5 { r.p_home } else { 1.0 - r.p_home };
        let fav_won = if r.p_home >= 0.5 { r.home_won } else { !r.home_won };
        let i = ((fav - 0.50) / 0.05).floor().clamp(0.0, 9.0) as usize;
        buckets[i].0 += fav;
        buckets[i].1 += if fav_won { 1.0 } else { 0.0 };
        buckets[i].2 += 1;
    }
    buckets
        .into_iter()
        .enumerate()
        .filter(|(_, (_, _, n))| *n > 0)
        .map(|(i, (ps, ys, n))| {
            let lo = 50 + i * 5;
            CalibBucket {
                label: format!("{lo}–{}", lo + 5),
                predicted: round_to(ps / n as f64, 3),
                actual: round_to(ys / n as f64, 3),
                n,
            }
        })
        .collect()
}

fn monthly(rows: &[BacktestGame]) -> Vec<MonthRow> {
    let mut map: HashMap<String, Vec<&BacktestGame>> = HashMap::new();
    for r in rows {
        let m = r.date.get(..7).unwrap_or(&r.date).to_string();
        map.entry(m).or_default().push(r);
    }
    let mut keys: Vec<_> = map.keys().cloned().collect();
    keys.sort();
    keys.into_iter()
        .map(|month| {
            let rs = &map[&month];
            let n = rs.len() as f64;
            MonthRow {
                month,
                n: rs.len() as u32,
                hit_rate: round_to(rs.iter().filter(|r| r.hit).count() as f64 / n, 4),
                brier: round_to(rs.iter().map(|r| r.brier).sum::<f64>() / n, 4),
                total_runs_mae: round_to(
                    rs.iter()
                        .map(|r| {
                            ((r.pred_home_runs + r.pred_away_runs)
                                - (r.actual_home_runs + r.actual_away_runs) as f64)
                                .abs()
                        })
                        .sum::<f64>()
                        / n,
                    3,
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mlb_api::{Game, GameStatus};

    fn fin(pk: i64, date: &str, h_id: i32, h: &str, hr: i32, a_id: i32, a: &str, ar: i32) -> Game {
        Game {
            game_pk: pk,
            date: date.into(),
            game_date_time: None,
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
        }
    }

    fn series(start_pk: i64, date: &str, h_id: i32, h: &str, a_id: i32, a: &str, hr: i32, ar: i32, n: i64) -> Vec<Game> {
        (0..n)
            .map(|i| {
                fin(
                    start_pk + i,
                    date,
                    h_id,
                    h,
                    hr,
                    a_id,
                    a,
                    ar,
                )
            })
            .collect()
    }

    #[test]
    fn scoring_day_does_not_see_that_days_runs() {
        // 10 identical A-home blowouts on day 1 so both teams have 10 GP.
        let mut games = series(1, "2026-04-01", 1, "A", 2, "B", 10, 1, 10);
        // Day 2: A is still a juggernaut if we leak day-2... we add a B-home blowout
        // that would tank A's Pythagorean if leaked into priors.
        games.push(fin(100, "2026-04-02", 2, "B", 20, 1, "A", 0));
        let bundle = run_backtest(2026, &games, &[], false, false, false);
        assert_eq!(bundle.n, 1, "only the day-2 game is scorable");
        // Priors: A scored 100, allowed 10 over 10 games. Strong favorite at home.
        assert!(bundle.games[0].p_home < 0.5, "A is away on day 2; B should be favored from day-1 priors, p_home={}", bundle.games[0].p_home);
    }

    #[test]
    fn favorite_hit_and_run_error() {
        let mut games = series(1, "2026-04-01", 1, "A", 2, "B", 8, 2, 10);
        games.push(fin(50, "2026-04-02", 1, "A", 7, 2, "B", 1));
        let bundle = run_backtest(2026, &games, &[], false, false, false);
        assert_eq!(bundle.n, 1);
        assert_eq!(bundle.skipped_early, 10);
        assert_eq!(bundle.skipped_tied, 0);
        assert!(bundle.games[0].hit);
        assert!(bundle.games[0].picked_home);
        assert_eq!(bundle.hit_rate, 1.0);
        assert!(bundle.total_runs_mae >= 0.0);
    }

    #[test]
    fn era_as_of_ignores_the_start_being_scored() {
        let d = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let logs = vec![
            StarterLog {
                pitcher_id: 9,
                game_pk: 1,
                game_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                home: true,
                innings_pitched: 30.0,
                earned_runs: 10.0,
            },
            StarterLog {
                pitcher_id: 9,
                game_pk: 99,
                game_date: d,
                home: true,
                innings_pitched: 9.0,
                earned_runs: 20.0,
            },
        ];
        let (era, ip) = era_as_of(&logs, 9, d).unwrap();
        assert!((ip - 30.0).abs() < 1e-9);
        assert!((era - 3.0).abs() < 1e-9);
    }

    #[test]
    fn suspended_resume_same_pk_scored_once() {
        let mut games = series(1, "2026-04-01", 1, "A", 2, "B", 8, 2, 10);
        games.push(fin(824912, "2026-06-16", 1, "A", 2, 2, "B", 7));
        games.push(fin(824912, "2026-06-16", 1, "A", 2, 2, "B", 7));
        let bundle = run_backtest(2026, &games, &[], false, false, false);
        assert_eq!(bundle.n, 1);
        assert_eq!(bundle.games.iter().filter(|g| g.game_pk == 824912).count(), 1);
    }

    #[test]
    fn tied_box_score_is_not_a_pick() {
        let mut games = series(1, "2026-04-01", 1, "A", 2, "B", 8, 2, 10);
        games.push(fin(50, "2026-04-02", 1, "A", 3, 2, "B", 3));
        let bundle = run_backtest(2026, &games, &[], false, false, false);
        assert_eq!(bundle.n, 0);
        assert_eq!(bundle.skipped_tied, 1);
        assert_eq!(bundle.skipped_early, 10);
    }
}
