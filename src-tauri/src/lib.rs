// Tauri entrypoint + command registry.
// Math + API live in model.rs and mlb_api.rs.

pub mod mlb_api;
pub mod model;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use mlb_api::{fetch_pitcher_stats, fetch_schedule, Game, PitcherStats};
use model::{
    compute_team_stats, estimate_game, estimate_game_with_pitchers, optimize_exponent,
    prob_to_american_odds, pythag_win_pct, GameRow, PitcherAdj, PitcherInfo, Prediction, TeamStats,
    MIN_IP_FOR_ADJUSTMENT,
};

const CACHE_TTL: Duration = Duration::from_secs(600); // 10 minutes
const PITCHER_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour — ERA changes slowly

pub struct AppState {
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    schedule: Option<(i32, Vec<Game>, Instant)>,
    optimal_exp: HashMap<i32, f64>,
    pitchers: HashMap<i32, (PitcherStats, Instant)>,
}

impl AppState {
    fn new() -> Self {
        Self {
            cache: Mutex::new(Cache::default()),
        }
    }

    async fn get_games(&self, season: i32, force: bool) -> Result<Vec<Game>, String> {
        if !force {
            let cached = {
                let cache = self.cache.lock().unwrap();
                cache.schedule.as_ref().and_then(|(s, games, t)| {
                    if *s == season && t.elapsed() < CACHE_TTL {
                        Some(games.clone())
                    } else {
                        None
                    }
                })
            };
            if let Some(games) = cached {
                return Ok(games);
            }
        }
        let games = fetch_schedule(season).await.map_err(|e| e.to_string())?;
        let mut cache = self.cache.lock().unwrap();
        cache.schedule = Some((season, games.clone(), Instant::now()));
        cache.optimal_exp.remove(&season); // invalidate
        Ok(games)
    }

    fn get_or_compute_optimal_exp(&self, season: i32, games: &[Game]) -> f64 {
        {
            let cache = self.cache.lock().unwrap();
            if let Some(v) = cache.optimal_exp.get(&season) {
                return *v;
            }
        }
        let exp = optimize_exponent(games);
        let mut cache = self.cache.lock().unwrap();
        cache.optimal_exp.insert(season, exp);
        exp
    }

    // Look up pitcher stats from cache; fetch any missing IDs from the API.
    async fn get_pitchers(
        &self,
        season: i32,
        ids: &[i32],
    ) -> Result<HashMap<i32, PitcherStats>, String> {
        let mut found = HashMap::new();
        let mut missing = Vec::new();
        {
            let cache = self.cache.lock().unwrap();
            for id in ids {
                if let Some((ps, t)) = cache.pitchers.get(id) {
                    if t.elapsed() < PITCHER_CACHE_TTL {
                        found.insert(*id, ps.clone());
                        continue;
                    }
                }
                missing.push(*id);
            }
        }
        if !missing.is_empty() {
            let fetched = fetch_pitcher_stats(season, &missing)
                .await
                .map_err(|e| e.to_string())?;
            let now = Instant::now();
            let mut cache = self.cache.lock().unwrap();
            for (id, ps) in fetched.iter() {
                cache.pitchers.insert(*id, (ps.clone(), now));
            }
            found.extend(fetched);
        }
        Ok(found)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PredictionsBundle {
    season: i32,
    date: String,
    exponent: f64,
    league_avg_runs: f64,
    last_updated: String,
    games: Vec<GameRow>,
    skipped: Vec<String>,
    available_dates: Vec<String>,
}

#[tauri::command]
async fn get_predictions(
    state: State<'_, AppState>,
    season: Option<i32>,
    date: Option<String>,
    exponent: Option<f64>,
    include_pitchers: Option<bool>,
) -> Result<PredictionsBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let include_pitchers = include_pitchers.unwrap_or(true);
    let games = state.get_games(season, false).await?;

    let exp = match exponent {
        Some(e) => e,
        None => state.get_or_compute_optimal_exp(season, &games),
    };

    let (team_stats, lg_avg_runs) = compute_team_stats(&games, exp);
    let team_by_id: HashMap<i32, &TeamStats> =
        team_stats.iter().map(|t| (t.team_id, t)).collect();

    let target_date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

    // Collect every probable pitcher for the target date, fetch their stats once.
    // When include_pitchers is false the user wants pure Pythagorean — skip the fetch entirely.
    let pitchers: HashMap<i32, PitcherStats> = if include_pitchers {
        let pitcher_ids: Vec<i32> = games
            .iter()
            .filter(|g| g.date == target_date)
            .flat_map(|g| [g.home_pitcher_id, g.away_pitcher_id])
            .flatten()
            .collect();
        let mut unique_ids = pitcher_ids.clone();
        unique_ids.sort_unstable();
        unique_ids.dedup();
        state.get_pitchers(season, &unique_ids).await.unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for g in games.iter().filter(|g| g.date == target_date) {
        let home = team_by_id.get(&g.home_team_id);
        let away = team_by_id.get(&g.away_team_id);
        match (home, away) {
            (Some(h), Some(a)) => {
                let home_p_adj = g
                    .home_pitcher_id
                    .and_then(|id| pitchers.get(&id))
                    .map(|p| PitcherAdj {
                        era: p.era,
                        innings_pitched: p.innings_pitched,
                    });
                let away_p_adj = g
                    .away_pitcher_id
                    .and_then(|id| pitchers.get(&id))
                    .map(|p| PitcherAdj {
                        era: p.era,
                        innings_pitched: p.innings_pitched,
                    });
                let pred = estimate_game_with_pitchers(
                    h,
                    a,
                    lg_avg_runs,
                    home_p_adj,
                    away_p_adj,
                    exp,
                );
                let (home_pinfo, away_pinfo) = if include_pitchers {
                    (
                        pitcher_info(g.home_pitcher_id, g.home_pitcher_name.as_deref(), &pitchers),
                        pitcher_info(g.away_pitcher_id, g.away_pitcher_name.as_deref(), &pitchers),
                    )
                } else {
                    (None, None)
                };
                rows.push(GameRow {
                    date: g.date.clone(),
                    home: g.home_team_name.clone(),
                    away: g.away_team_name.clone(),
                    home_pitcher: home_pinfo,
                    away_pitcher: away_pinfo,
                    pred,
                });
            }
            _ => {
                skipped.push(format!("{} @ {}", g.away_team_name, g.home_team_name));
            }
        }
    }

    let mut available_dates: Vec<String> = games
        .iter()
        .filter(|g| matches!(g.status, mlb_api::GameStatus::Preview | mlb_api::GameStatus::Live))
        .map(|g| g.date.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    available_dates.sort();

    Ok(PredictionsBundle {
        season,
        date: target_date,
        exponent: round_to(exp, 4),
        league_avg_runs: round_to(lg_avg_runs, 3),
        last_updated: Utc::now().to_rfc3339(),
        games: rows,
        skipped,
        available_dates,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TeamStatsBundle {
    season: i32,
    exponent: f64,
    league_avg_runs: f64,
    optimal_exponent: f64,
    teams: Vec<TeamStats>,
}

#[tauri::command]
async fn get_team_stats(
    state: State<'_, AppState>,
    season: Option<i32>,
    exponent: Option<f64>,
) -> Result<TeamStatsBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let games = state.get_games(season, false).await?;
    let optimal = state.get_or_compute_optimal_exp(season, &games);
    let exp = exponent.unwrap_or(optimal);
    let (stats, lg_avg_runs) = compute_team_stats(&games, exp);
    Ok(TeamStatsBundle {
        season,
        exponent: round_to(exp, 4),
        league_avg_runs: round_to(lg_avg_runs, 3),
        optimal_exponent: round_to(optimal, 4),
        teams: stats,
    })
}

#[tauri::command]
async fn get_optimal_exponent(
    state: State<'_, AppState>,
    season: Option<i32>,
) -> Result<f64, String> {
    let season = season.unwrap_or_else(default_season);
    let games = state.get_games(season, false).await?;
    Ok(round_to(state.get_or_compute_optimal_exp(season, &games), 4))
}

#[tauri::command]
async fn refresh_schedule(
    state: State<'_, AppState>,
    season: Option<i32>,
) -> Result<usize, String> {
    let season = season.unwrap_or_else(default_season);
    let games = state.get_games(season, true).await?;
    Ok(games.len())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamInput {
    pub runs_scored: f64,
    pub runs_allowed: f64,
    pub games: f64,
}

#[tauri::command]
fn compute_matchup(
    home: TeamInput,
    away: TeamInput,
    exponent: f64,
    league_avg_runs: f64,
) -> Prediction {
    let h = make_synth_team(&home, exponent, league_avg_runs);
    let a = make_synth_team(&away, exponent, league_avg_runs);
    estimate_game(&h, &a, league_avg_runs)
}

#[tauri::command]
fn pythag_curve(
    runs_scored: f64,
    runs_allowed: f64,
    min_exp: f64,
    max_exp: f64,
    steps: u32,
) -> Vec<(f64, f64)> {
    // Returns [(exponent, win%)] over [min_exp, max_exp] — for the sensitivity chart.
    let steps = steps.max(2);
    let step = (max_exp - min_exp) / (steps as f64 - 1.0);
    (0..steps)
        .map(|i| {
            let e = min_exp + step * i as f64;
            (round_to(e, 4), round_to(pythag_win_pct(runs_scored, runs_allowed, e), 6))
        })
        .collect()
}

#[tauri::command]
fn american_odds(prob: f64) -> i32 {
    prob_to_american_odds(prob)
}

fn make_synth_team(t: &TeamInput, exponent: f64, lg_avg_runs: f64) -> TeamStats {
    let gp = if t.games <= 0.0 { 1.0 } else { t.games };
    TeamStats {
        team_id: 0,
        team: String::new(),
        runs_scored: t.runs_scored as i32,
        runs_allowed: t.runs_allowed as i32,
        games_played: gp as i32,
        pythag_win_pct: pythag_win_pct(t.runs_scored, t.runs_allowed, exponent),
        os: (t.runs_scored / gp) / lg_avg_runs,
        ds: (t.runs_allowed / gp) / lg_avg_runs,
    }
}

fn default_season() -> i32 {
    Utc::now()
        .date_naive()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2025)
}

fn round_to(x: f64, places: u32) -> f64 {
    let m = 10f64.powi(places as i32);
    (x * m).round() / m
}

// Build a PitcherInfo row for the UI. Falls back to a TBD-style entry when the
// pitcher is announced but we have no season stats yet (rare early-season case).
fn pitcher_info(
    id: Option<i32>,
    name: Option<&str>,
    pitchers: &HashMap<i32, PitcherStats>,
) -> Option<PitcherInfo> {
    let id = id?;
    let name = name.unwrap_or("Unknown").to_string();
    match pitchers.get(&id) {
        Some(ps) => Some(PitcherInfo {
            name,
            era: round_to(ps.era, 2),
            innings_pitched: round_to(ps.innings_pitched, 1),
            games_started: ps.games_started,
            applied: ps.innings_pitched >= MIN_IP_FOR_ADJUSTMENT,
        }),
        None => Some(PitcherInfo {
            name,
            era: 0.0,
            innings_pitched: 0.0,
            games_started: 0,
            applied: false,
        }),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_predictions,
            get_team_stats,
            get_optimal_exponent,
            refresh_schedule,
            compute_matchup,
            pythag_curve,
            american_odds,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
