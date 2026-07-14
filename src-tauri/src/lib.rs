// Tauri entrypoint + command registry.
// Math + API live in model.rs and mlb_api.rs.

pub mod mlb_api;
pub mod model;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use tauri::State;

use mlb_api::{
    fetch_boxscore, fetch_bullpen, fetch_pitcher_stats, fetch_schedule, fetch_standings, Bullpen,
    Game, Lineups, PitcherStats, TeamStanding,
};
use model::{
    compute_head_to_head, compute_recent_form, compute_splits, compute_team_stats,
    estimate_game_detailed, estimate_game_with_pitchers, optimize_exponent, round_to, GameRow,
    HeadToHead, MatchupBreakdown, PitcherAdj, PitcherInfo, Prediction, RecentForm, RecentInfo,
    TeamSplits, TeamStats, MIN_IP_FOR_ADJUSTMENT, MIN_RECENT_GAMES, RECENT_FORM_WINDOW,
};

const CACHE_TTL: Duration = Duration::from_secs(600); // 10 minutes
const PITCHER_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour — ERA changes slowly
const STANDINGS_CACHE_TTL: Duration = Duration::from_secs(600); // 10 minutes
const BOXSCORE_CACHE_TTL: Duration = Duration::from_secs(300); // 5 min — lineups shift until first pitch
const BULLPEN_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

pub struct AppState {
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    schedule: Option<(i32, Vec<Game>, Instant)>,
    optimal_exp: HashMap<i32, f64>,
    pitchers: HashMap<(i32, i32), (PitcherStats, Instant)>,
    standings: Option<(i32, Vec<TeamStanding>, Instant)>,
    boxscores: HashMap<i64, (Lineups, Instant)>,
    bullpens: HashMap<(i32, i32), (Bullpen, Instant)>,
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
                if let Some((ps, t)) = cache.pitchers.get(&(season, *id)) {
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
            for id in &missing {
                match fetched.get(id) {
                    Some(ps) => {
                        cache.pitchers.insert((season, *id), (ps.clone(), now));
                    }
                    None => {
                        // We asked for this pitcher but the API returned no stats
                        // (e.g. mid-season trade, IL stint with no data). Drop any
                        // stale cached entry so we don't keep refetching every call —
                        // they'll be re-added if/when stats reappear.
                        cache.pitchers.remove(&(season, *id));
                    }
                }
            }
            found.extend(fetched);
        }
        Ok(found)
    }

    async fn get_standings(&self, season: i32, force: bool) -> Result<Vec<TeamStanding>, String> {
        if !force {
            let cached = {
                let cache = self.cache.lock().unwrap();
                cache.standings.as_ref().and_then(|(s, st, t)| {
                    if *s == season && t.elapsed() < STANDINGS_CACHE_TTL {
                        Some(st.clone())
                    } else {
                        None
                    }
                })
            };
            if let Some(st) = cached {
                return Ok(st);
            }
        }
        let st = fetch_standings(season).await.map_err(|e| e.to_string())?;
        let mut cache = self.cache.lock().unwrap();
        cache.standings = Some((season, st.clone(), Instant::now()));
        Ok(st)
    }

    async fn get_boxscore(&self, game_pk: i64) -> Result<Lineups, String> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some((lu, t)) = cache.boxscores.get(&game_pk) {
                if t.elapsed() < BOXSCORE_CACHE_TTL {
                    return Ok(lu.clone());
                }
            }
        }
        let lu = fetch_boxscore(game_pk).await.map_err(|e| e.to_string())?;
        let mut cache = self.cache.lock().unwrap();
        cache.boxscores.insert(game_pk, (lu.clone(), Instant::now()));
        Ok(lu)
    }

    async fn get_bullpen(&self, season: i32, team_id: i32) -> Result<Option<Bullpen>, String> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some((bp, t)) = cache.bullpens.get(&(season, team_id)) {
                if t.elapsed() < BULLPEN_CACHE_TTL {
                    return Ok(Some(*bp));
                }
            }
        }
        let bp = fetch_bullpen(season, team_id).await.map_err(|e| e.to_string())?;
        // Only cache a real hit — leave None uncached so it retries when the split appears.
        if let Some(bp) = bp {
            let mut cache = self.cache.lock().unwrap();
            cache.bullpens.insert((season, team_id), (bp, Instant::now()));
        }
        Ok(bp)
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
    include_home_field: Option<bool>,
    include_recent_form: Option<bool>,
) -> Result<PredictionsBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let include_pitchers = include_pitchers.unwrap_or(true);
    let include_home_field = include_home_field.unwrap_or(true);
    let include_recent_form = include_recent_form.unwrap_or(true);
    let games = state.get_games(season, false).await?;

    let exp = match exponent {
        Some(e) => e,
        None => state.get_or_compute_optimal_exp(season, &games),
    };

    let (team_stats, lg_avg_runs) = compute_team_stats(&games, exp);
    let team_by_id: HashMap<i32, &TeamStats> =
        team_stats.iter().map(|t| (t.team_id, t)).collect();

    // Always compute recent-form so we can DISPLAY the L20 line on every card.
    // The include_recent_form toggle gates whether it enters the model math.
    let recent_by_id = compute_recent_form(&games, RECENT_FORM_WINDOW);

    let target_date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

    // Always fetch probable-pitcher stats so we can DISPLAY them on the card.
    // The include_pitchers toggle gates whether they enter the model math (below),
    // not whether the user sees who's starting.
    let pitcher_ids: Vec<i32> = games
        .iter()
        .filter(|g| g.date == target_date)
        .flat_map(|g| [g.home_pitcher_id, g.away_pitcher_id])
        .flatten()
        .collect();
    let mut unique_ids = pitcher_ids.clone();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    let pitchers = match state.get_pitchers(season, &unique_ids).await {
        Ok(p) => p,
        Err(e) => {
            // The MLB people endpoint failed — fall back to no pitcher adjustment
            // (frontend will show names only, math runs pure Pythagorean). Log so
            // the dev console / Tauri log surfaces a persistent outage.
            eprintln!("[mlb-pe] pitcher stats fetch failed: {}", e);
            HashMap::new()
        }
    };

    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for g in games.iter().filter(|g| g.date == target_date) {
        let home = team_by_id.get(&g.home_team_id);
        let away = team_by_id.get(&g.away_team_id);
        match (home, away) {
            (Some(h), Some(a)) => {
                let home_recent_raw = recent_by_id.get(&g.home_team_id).copied();
                let away_recent_raw = recent_by_id.get(&g.away_team_id).copied();
                let (home_p_adj, away_p_adj, home_recent_adj, away_recent_adj) = matchup_inputs(
                    g,
                    &pitchers,
                    &recent_by_id,
                    include_pitchers,
                    include_recent_form,
                );

                let pred = estimate_game_with_pitchers(
                    h,
                    a,
                    lg_avg_runs,
                    home_p_adj,
                    away_p_adj,
                    home_recent_adj,
                    away_recent_adj,
                    exp,
                    include_home_field,
                );
                // Always surface pitcher info for display, but mark `applied = false`
                // when the toggle is off so the UI can fade the ERA visually.
                let home_pinfo = pitcher_info(
                    g.home_pitcher_id,
                    g.home_pitcher_name.as_deref(),
                    &pitchers,
                    include_pitchers,
                );
                let away_pinfo = pitcher_info(
                    g.away_pitcher_id,
                    g.away_pitcher_name.as_deref(),
                    &pitchers,
                    include_pitchers,
                );
                let home_rinfo = recent_info(home_recent_raw, include_recent_form);
                let away_rinfo = recent_info(away_recent_raw, include_recent_form);
                rows.push(GameRow {
                    game_pk: g.game_pk,
                    date: g.date.clone(),
                    home: g.home_team_name.clone(),
                    away: g.away_team_name.clone(),
                    home_pitcher: home_pinfo,
                    away_pitcher: away_pinfo,
                    home_recent: home_rinfo,
                    away_recent: away_rinfo,
                    pred,
                });
            }
            _ => {
                skipped.push(format!("{} @ {}", g.away_team_name, g.home_team_name));
            }
        }
    }

    // BTreeSet iterates in sorted order, so the resulting Vec is already sorted.
    let available_dates: Vec<String> = games
        .iter()
        .filter(|g| matches!(g.status, mlb_api::GameStatus::Preview | mlb_api::GameStatus::Live))
        .map(|g| g.date.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StandingsBundle {
    season: i32,
    last_updated: String,
    teams: Vec<TeamStanding>,
}

#[tauri::command]
async fn get_standings(
    state: State<'_, AppState>,
    season: Option<i32>,
) -> Result<StandingsBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let teams = state.get_standings(season, false).await?;
    Ok(StandingsBundle {
        season,
        last_updated: Utc::now().to_rfc3339(),
        teams,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameBreakdownBundle {
    season: i32,
    date: String,
    game_pk: i64,
    home: String,
    away: String,
    home_team_id: i32,
    away_team_id: i32,
    home_pitcher: Option<PitcherInfo>,
    away_pitcher: Option<PitcherInfo>,
    home_recent: Option<RecentInfo>,
    away_recent: Option<RecentInfo>,
    prediction: Prediction,
    breakdown: MatchupBreakdown,
}

#[tauri::command]
async fn get_game_breakdown(
    state: State<'_, AppState>,
    season: Option<i32>,
    game_pk: i64,
    exponent: Option<f64>,
    include_pitchers: Option<bool>,
    include_home_field: Option<bool>,
    include_recent_form: Option<bool>,
) -> Result<GameBreakdownBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let include_pitchers = include_pitchers.unwrap_or(true);
    let include_home_field = include_home_field.unwrap_or(true);
    let include_recent_form = include_recent_form.unwrap_or(true);
    let games = state.get_games(season, false).await?;

    let g = games
        .iter()
        .find(|g| g.game_pk == game_pk)
        .ok_or_else(|| format!("game {} not found in the {} schedule", game_pk, season))?;

    let exp = match exponent {
        Some(e) => e,
        None => state.get_or_compute_optimal_exp(season, &games),
    };

    let (team_stats, lg_avg_runs) = compute_team_stats(&games, exp);
    let team_by_id: HashMap<i32, &TeamStats> =
        team_stats.iter().map(|t| (t.team_id, t)).collect();
    let recent_by_id = compute_recent_form(&games, RECENT_FORM_WINDOW);

    let home = team_by_id
        .get(&g.home_team_id)
        .ok_or_else(|| format!("no season stats yet for {}", g.home_team_name))?;
    let away = team_by_id
        .get(&g.away_team_id)
        .ok_or_else(|| format!("no season stats yet for {}", g.away_team_name))?;

    let mut ids: Vec<i32> = [g.home_pitcher_id, g.away_pitcher_id]
        .into_iter()
        .flatten()
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let pitchers = match state.get_pitchers(season, &ids).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[mlb-pe] pitcher stats fetch failed: {}", e);
            HashMap::new()
        }
    };

    let home_recent_raw = recent_by_id.get(&g.home_team_id).copied();
    let away_recent_raw = recent_by_id.get(&g.away_team_id).copied();
    let (home_p_adj, away_p_adj, home_recent_adj, away_recent_adj) =
        matchup_inputs(g, &pitchers, &recent_by_id, include_pitchers, include_recent_form);

    let (prediction, breakdown) = estimate_game_detailed(
        home,
        away,
        lg_avg_runs,
        home_p_adj,
        away_p_adj,
        home_recent_adj,
        away_recent_adj,
        exp,
        include_home_field,
    );

    Ok(GameBreakdownBundle {
        season,
        date: g.date.clone(),
        game_pk,
        home: g.home_team_name.clone(),
        away: g.away_team_name.clone(),
        home_team_id: g.home_team_id,
        away_team_id: g.away_team_id,
        home_pitcher: pitcher_info(
            g.home_pitcher_id,
            g.home_pitcher_name.as_deref(),
            &pitchers,
            include_pitchers,
        ),
        away_pitcher: pitcher_info(
            g.away_pitcher_id,
            g.away_pitcher_name.as_deref(),
            &pitchers,
            include_pitchers,
        ),
        home_recent: recent_info(home_recent_raw, include_recent_form),
        away_recent: recent_info(away_recent_raw, include_recent_form),
        prediction,
        breakdown,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameContextBundle {
    game_pk: i64,
    home: String,
    away: String,
    home_team_id: i32,
    away_team_id: i32,
    head_to_head: HeadToHead,
    home_splits: TeamSplits,
    away_splits: TeamSplits,
    lineups: Lineups,
    home_bullpen: Option<Bullpen>,
    away_bullpen: Option<Bullpen>,
}

// Matchup + team context around a game: head-to-head series and home/road/L10
// splits (computed from the cached schedule, no network) plus probable lineups
// and each bullpen's relief line (fetched, cached, best-effort). Loaded by the
// detail page separately from get_game_breakdown so the model walkthrough never
// waits on these network calls.
#[tauri::command]
async fn get_game_context(
    state: State<'_, AppState>,
    season: Option<i32>,
    game_pk: i64,
) -> Result<GameContextBundle, String> {
    let season = season.unwrap_or_else(default_season);
    let games = state.get_games(season, false).await?;
    let g = games
        .iter()
        .find(|g| g.game_pk == game_pk)
        .ok_or_else(|| format!("game {} not found in the {} schedule", game_pk, season))?;

    let head_to_head = compute_head_to_head(&games, g.home_team_id, g.away_team_id);
    let home_splits = compute_splits(&games, g.home_team_id);
    let away_splits = compute_splits(&games, g.away_team_id);

    // Network calls run concurrently; each degrades to empty/None rather than
    // failing the whole bundle (same posture as the pitcher-fetch fallback).
    let (lineups, home_bp, away_bp) = tokio::join!(
        state.get_boxscore(game_pk),
        state.get_bullpen(season, g.home_team_id),
        state.get_bullpen(season, g.away_team_id),
    );
    let lineups = lineups.unwrap_or_else(|e| {
        eprintln!("[mlb-pe] boxscore fetch failed: {}", e);
        Lineups::default()
    });
    let home_bullpen = home_bp.unwrap_or_else(|e| {
        eprintln!("[mlb-pe] home bullpen fetch failed: {}", e);
        None
    });
    let away_bullpen = away_bp.unwrap_or_else(|e| {
        eprintln!("[mlb-pe] away bullpen fetch failed: {}", e);
        None
    });

    Ok(GameContextBundle {
        game_pk,
        home: g.home_team_name.clone(),
        away: g.away_team_name.clone(),
        home_team_id: g.home_team_id,
        away_team_id: g.away_team_id,
        head_to_head,
        home_splits,
        away_splits,
        lineups,
        home_bullpen,
        away_bullpen,
    })
}

fn default_season() -> i32 {
    Utc::now()
        .date_naive()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2026)
}

// Toggle-gated model inputs for one game: the starter adjustments and L20 forms
// that actually feed estimate_game_*. Returns None for any input the toggle
// disables (so the prediction collapses to pure team-level Pythagorean). Shared
// by get_predictions and get_game_breakdown so the gating can never drift.
fn matchup_inputs(
    g: &Game,
    pitchers: &HashMap<i32, PitcherStats>,
    recent_by_id: &HashMap<i32, RecentForm>,
    include_pitchers: bool,
    include_recent_form: bool,
) -> (
    Option<PitcherAdj>,
    Option<PitcherAdj>,
    Option<RecentForm>,
    Option<RecentForm>,
) {
    let p_adj = |id: Option<i32>| -> Option<PitcherAdj> {
        if !include_pitchers {
            return None;
        }
        id.and_then(|id| pitchers.get(&id)).map(|p| PitcherAdj {
            era: p.era,
            innings_pitched: p.innings_pitched,
        })
    };
    let recent = |team_id: i32| -> Option<RecentForm> {
        if include_recent_form {
            recent_by_id.get(&team_id).copied()
        } else {
            None
        }
    };
    (
        p_adj(g.home_pitcher_id),
        p_adj(g.away_pitcher_id),
        recent(g.home_team_id),
        recent(g.away_team_id),
    )
}

// Build a PitcherInfo row for the UI. `model_enabled` reflects the toggle:
// false means the pitcher is shown but the prediction is pure team-level Pythagorean.
fn pitcher_info(
    id: Option<i32>,
    name: Option<&str>,
    pitchers: &HashMap<i32, PitcherStats>,
    model_enabled: bool,
) -> Option<PitcherInfo> {
    let id = id?;
    let name = name.unwrap_or("Unknown").to_string();
    match pitchers.get(&id) {
        Some(ps) => {
            let eligible = ps.innings_pitched >= MIN_IP_FOR_ADJUSTMENT;
            Some(PitcherInfo {
                name,
                era: round_to(ps.era, 2),
                innings_pitched: round_to(ps.innings_pitched, 1),
                games_started: ps.games_started,
                applied: model_enabled && eligible,
                eligible_sample: eligible,
            })
        }
        None => Some(PitcherInfo {
            name,
            era: 0.0,
            innings_pitched: 0.0,
            games_started: 0,
            applied: false,
            eligible_sample: false,
        }),
    }
}

// Mirror of pitcher_info for the L20 line. Always returned when we have any
// completed games for the team; `applied` reflects toggle + sample threshold.
fn recent_info(recent: Option<RecentForm>, model_enabled: bool) -> Option<RecentInfo> {
    let r = recent?;
    let eligible = r.games >= MIN_RECENT_GAMES;
    Some(RecentInfo {
        games: r.games,
        rs_per_game: round_to(r.rs_per_game, 2),
        ra_per_game: round_to(r.ra_per_game, 2),
        applied: model_enabled && eligible,
        eligible_sample: eligible,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_predictions,
            get_game_breakdown,
            get_game_context,
            get_team_stats,
            get_optimal_exponent,
            refresh_schedule,
            get_standings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
