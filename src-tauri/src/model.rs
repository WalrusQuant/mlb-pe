// Pythagorean Expectation + log5 + run-environment model.
// Direct port of the two R scripts in legacy/, parameterized so callers can sweep the exponent.

use std::collections::HashMap;

use serde::Serialize;

use crate::mlb_api::Game;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStats {
    pub team_id: i32,
    pub team: String,
    pub runs_scored: i32,
    pub runs_allowed: i32,
    pub games_played: i32,
    pub pythag_win_pct: f64,
    pub os: f64,
    pub ds: f64,
    // L20 (or however many completed games we have, up to RECENT_FORM_WINDOW).
    // None when the team has no completed games yet.
    pub recent_games: Option<i32>,
    pub recent_rs_per_game: Option<f64>,
    pub recent_ra_per_game: Option<f64>,
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
    // "nextStart" | "era" | "none" — what actually entered apply_pitcher.
    pub blend_source: String,
    pub projected_fip: Option<f64>,
    pub expected_runs: Option<f64>,
    pub expected_runs_low: Option<f64>,
    pub expected_runs_high: Option<f64>,
    pub expected_innings: Option<f64>,
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentInfo {
    pub games: i32,
    pub rs_per_game: f64,
    pub ra_per_game: f64,
    // True when the L20 sample actually shifted the prediction.
    // False if the toggle is off OR games < MIN_RECENT_GAMES.
    pub applied: bool,
    pub eligible_sample: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameRow {
    #[serde(rename = "gamePk")]
    pub game_pk: i64,
    pub date: String,
    #[serde(rename = "gameDateTime")]
    pub game_date_time: Option<String>,
    #[serde(rename = "homeTeamId")]
    pub home_team_id: i32,
    #[serde(rename = "awayTeamId")]
    pub away_team_id: i32,
    pub home: String,
    pub away: String,
    #[serde(rename = "homePitcher")]
    pub home_pitcher: Option<PitcherInfo>,
    #[serde(rename = "awayPitcher")]
    pub away_pitcher: Option<PitcherInfo>,
    #[serde(rename = "homeRecent")]
    pub home_recent: Option<RecentInfo>,
    #[serde(rename = "awayRecent")]
    pub away_recent: Option<RecentInfo>,
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

/// P(home scores more than away | not tied) if each side is independent Poisson
/// at the predicted run line. Ties (same inning-by-inning score after 9) are
/// dropped — extra innings are treated as "play until someone leads."
pub fn poisson_home_win(lambda_home: f64, lambda_away: f64) -> f64 {
    const MAX_K: usize = 40;
    score_home_win(
        &poisson_pmf(lambda_home.max(0.0), MAX_K),
        &poisson_pmf(lambda_away.max(0.0), MAX_K),
    )
}

/// Same as Poisson but with extra variance: Var = μ + μ²/r.
/// r = 2 ⇒ at μ = 4.5, Var ≈ 14.6 (~3× Poisson).
pub const NB_SIZE: f64 = 2.0;
/// Pull each side's predicted runs this far toward league average before
/// converting the run line to a win %. Displayed scores are not shrunk.
pub const RUN_LINE_SHRINK: f64 = 0.4;
/// Pull OS and DS this far toward 1.0 before OS×DS×lg. Changes the printed score.
pub const RATE_SHRINK: f64 = 0.4;

pub fn shrink_to_one(x: f64, s: f64) -> f64 {
    (1.0 - s) * x + s
}

/// 3-year run park factors (RotoWire 2023–2025, index/100). 1.0 = average.
/// Tonight's OS×DS line is multiplied by this when the Park toggle is on.
pub fn park_run_factor(venue_id: Option<i32>) -> f64 {
    match venue_id {
        Some(19) => 1.25,   // Coors Field
        Some(2529) => 1.17, // Sutter Health Park
        Some(3) => 1.10,    // Fenway Park
        Some(15) => 1.06,   // Chase Field
        Some(2602) => 1.06, // Great American Ball Park
        Some(3312) => 1.06, // Target Field
        Some(1) => 1.02,    // Angel Stadium
        Some(4705) => 1.02, // Truist Park
        Some(22) => 1.02,   // Dodger Stadium
        Some(4169) => 1.02, // loanDepot park
        Some(3309) => 1.02, // Nationals Park
        Some(2681) => 1.02, // Citizens Bank Park
        Some(7) => 1.02,    // Kauffman Stadium
        Some(2394) => 1.02, // Comerica Park
        Some(2392) => 1.00, // Daikin Park
        Some(14) => 1.00,   // Rogers Centre
        Some(2889) => 1.00, // Busch Stadium
        Some(3313) => 1.00, // Yankee Stadium
        Some(2) => 0.98,    // Camden Yards
        Some(31) => 0.98,   // PNC Park
        Some(4) => 0.98,    // Rate Field
        Some(3289) => 0.96, // Citi Field
        Some(32) => 0.94,   // American Family Field
        Some(17) => 0.94,   // Wrigley Field
        Some(2395) => 0.94, // Oracle Park
        Some(5) => 0.94,    // Progressive Field
        Some(2680) => 0.94, // Petco Park
        Some(5325) => 0.94, // Globe Life Field
        Some(12) => 0.92,   // Tropicana Field
        Some(680) => 0.83,  // T-Mobile Park
        _ => 1.0,
    }
}

pub fn shrink_run_line(pred: f64, lg_avg: f64) -> f64 {
    (1.0 - RUN_LINE_SHRINK) * pred + RUN_LINE_SHRINK * lg_avg
}

pub fn nb_home_win(mu_home: f64, mu_away: f64) -> f64 {
    const MAX_K: usize = 50;
    score_home_win(
        &nb_pmf(mu_home.max(0.0), NB_SIZE, MAX_K),
        &nb_pmf(mu_away.max(0.0), NB_SIZE, MAX_K),
    )
}

fn score_home_win(ph: &[f64], pa: &[f64]) -> f64 {
    let mut p_home = 0.0;
    let mut p_away = 0.0;
    for (i, &pi) in ph.iter().enumerate() {
        for (j, &pj) in pa.iter().enumerate() {
            let p = pi * pj;
            if i > j {
                p_home += p;
            } else if j > i {
                p_away += p;
            }
        }
    }
    let denom = p_home + p_away;
    if denom <= 0.0 {
        0.5
    } else {
        p_home / denom
    }
}

fn poisson_pmf(lambda: f64, max_k: usize) -> Vec<f64> {
    let mut p = vec![0.0; max_k + 1];
    p[0] = (-lambda).exp();
    for k in 0..max_k {
        p[k + 1] = p[k] * lambda / (k as f64 + 1.0);
    }
    p
}

fn nb_pmf(mu: f64, r: f64, max_k: usize) -> Vec<f64> {
    if r <= 0.0 {
        return poisson_pmf(mu, max_k);
    }
    let mut p = vec![0.0; max_k + 1];
    p[0] = (r / (r + mu)).powf(r);
    let t = mu / (r + mu);
    for k in 0..max_k {
        p[k + 1] = p[k] * (k as f64 + r) / (k as f64 + 1.0) * t;
    }
    p
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

// Aggregate season-to-date team stats from completed games. Each TeamStats also
// carries its team's L20 (or fewer, if not enough completed games) so callers
// can derive recency-aware views without a separate call.
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

    let recent = compute_recent_form(games, RECENT_FORM_WINDOW);

    let mut stats: Vec<TeamStats> = agg
        .into_iter()
        .filter(|(_, row)| row.games_played > 0)
        .map(|(team_id, row)| {
            let rs = row.runs_scored as f64;
            let ra = row.runs_allowed as f64;
            let gp = row.games_played as f64;
            let r = recent.get(&team_id);
            TeamStats {
                team_id,
                team: row.name,
                runs_scored: row.runs_scored as i32,
                runs_allowed: row.runs_allowed as i32,
                games_played: row.games_played as i32,
                pythag_win_pct: pythag_win_pct(rs, ra, exponent),
                os: (rs / gp) / lg_avg_runs,
                ds: (ra / gp) / lg_avg_runs,
                recent_games: r.map(|x| x.games),
                recent_rs_per_game: r.map(|x| round_to(x.rs_per_game, 2)),
                recent_ra_per_game: r.map(|x| round_to(x.ra_per_game, 2)),
            }
        })
        .collect();

    stats.sort_by(|a, b| a.team.cmp(&b.team));
    (stats, lg_avg_runs)
}

pub fn estimate_game(home: &TeamStats, away: &TeamStats, lg_avg_runs: f64) -> Prediction {
    estimate_game_with_pitchers(home, away, lg_avg_runs, None, None, None, None, 2.0, false)
}

// Per Bill James / sabermetrics consensus: a starting pitcher is responsible for ~5.4
// of 9 innings on average (~0.6 of the game's runs allowed). The remainder is the
// bullpen, which is implicitly captured in the team's season RA/G.
pub const STARTER_SHARE: f64 = 0.6;
// Below this, the pitcher sample is too small to trust — fall back to team RA.
pub const MIN_IP_FOR_ADJUSTMENT: f64 = 20.0;

// Home-field advantage: MLB's historical home win rate is ~54%. We add the
// equivalent log-odds shift to the home team's win probability:
//   logit(0.54) - logit(0.50) ≈ 0.1603
// Applied in log-odds space (not probability space) so it shrinks naturally at
// the extremes — a 90% favorite gets a smaller bump than a coin-flip game.
pub const HOME_FIELD_LOG_ODDS: f64 = 0.1603;

// Recent-form weighting: how many of a team's most recent completed games to
// pull into the blend, and how much weight to give them vs. the full season.
// Pythagorean weights April and September equally — a 60/40 season-heavy blend
// nudges the prediction toward how the team is *currently* playing without
// throwing away the larger sample.
pub const RECENT_FORM_WINDOW: usize = 20;
pub const RECENT_FORM_WEIGHT: f64 = 0.4;
// Below this many recent games we don't trust the L20 sample (early season,
// just back from a long break, etc.) — fall back to pure season stats.
pub const MIN_RECENT_GAMES: i32 = 10;

// Shift a win probability by a log-odds delta.
pub fn shift_log_odds(p: f64, delta: f64) -> f64 {
    // Clamp away from {0, 1} to avoid infinities. In practice log5 never returns
    // exactly 0 or 1 for any realistic input, but be defensive.
    let p = p.clamp(1e-9, 1.0 - 1e-9);
    let lo = logit(p) + delta;
    1.0 / (1.0 + (-lo).exp())
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-9, 1.0 - 1e-9);
    (p / (1.0 - p)).ln()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitcherSource {
    Era,
    NextStart,
}

#[derive(Debug, Clone, Copy)]
pub struct PitcherAdj {
    /// Season ERA (Era) or outing expected earned runs (NextStart).
    pub rate_or_er: f64,
    /// Season IP (Era, sample gate) or expected outing IP (NextStart, bullpen share).
    pub innings_pitched: f64,
    pub source: PitcherSource,
    /// When set, leftover innings use this rate instead of team RA/G.
    pub bullpen_ra: Option<f64>,
}

impl PitcherAdj {
    pub fn from_era(era: f64, innings_pitched: f64) -> Self {
        Self {
            rate_or_er: era,
            innings_pitched,
            source: PitcherSource::Era,
            bullpen_ra: None,
        }
    }

    pub fn from_next_start(expected_runs: f64, expected_innings: f64) -> Self {
        Self {
            rate_or_er: expected_runs,
            innings_pitched: expected_innings,
            source: PitcherSource::NextStart,
            bullpen_ra: None,
        }
    }

    pub fn is_applied(&self) -> bool {
        match self.source {
            PitcherSource::NextStart => true,
            PitcherSource::Era => self.innings_pitched >= MIN_IP_FOR_ADJUSTMENT,
        }
    }
}

/// Date-specific next-start projection from the vendored pns-core artifact.
#[derive(Debug, Clone)]
pub struct NextStartCard {
    pub projected_fip: f64,
    pub expected_runs: f64,
    pub expected_runs_low: f64,
    pub expected_runs_high: f64,
    pub expected_innings: f64,
    pub confidence: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentForm {
    pub games: i32,
    pub rs_per_game: f64,
    pub ra_per_game: f64,
}

// Per-team intermediate values behind the prediction, for the game-detail view.
// Everything here is already computed inside estimate_game_detailed; we just keep
// it instead of discarding it. rates flow: season -> (+recent blend) -> (+pitcher).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideBreakdown {
    pub season_rs_per_game: f64,
    pub season_ra_per_game: f64,
    // Recent-form (L20) inputs. present is false when the team has no L20 sample.
    pub recent_present: bool,
    pub recent_games: i32,
    pub recent_rs_per_game: f64,
    pub recent_ra_per_game: f64,
    // True when the L20 blend actually moved the rates (toggle on AND sample big enough).
    pub recent_applied: bool,
    pub blended_rs_per_game: f64,
    pub blended_ra_per_game: f64,
    // Pitcher inputs. present is false when no probable pitcher / no stats.
    pub pitcher_present: bool,
    pub pitcher_era: f64,
    pub pitcher_ip: f64,
    // True when the starter actually moved effective RA.
    pub pitcher_applied: bool,
    pub pitcher_source: &'static str,
    pub pitcher_expected_runs: f64,
    pub pitcher_expected_innings: f64,
    pub effective_ra_per_game: f64,
    pub pythag_win_pct: f64,
    pub os_eff: f64,
    pub ds_eff: f64,
    pub pred_runs: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchupBreakdown {
    pub home: SideBreakdown,
    pub away: SideBreakdown,
    pub exponent: f64,
    pub league_avg_runs: f64,
    // log5 home win before any home-field shift.
    pub neutral_home_win: f64,
    pub home_field_applied: bool,
    pub home_field_delta: f64,
    pub final_home_win: f64,
    pub final_away_win: f64,
}

// Aggregate each team's last N completed games into a RecentForm row.
// Iterates the schedule once in date order, then trims per team. Returns a
// map keyed by team_id so the predictions loop can look up O(1).
pub fn compute_recent_form(games: &[Game], window: usize) -> HashMap<i32, RecentForm> {
    let mut by_team: HashMap<i32, Vec<(String, i64, i64)>> = HashMap::new();
    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap() as i64;
        let ar = g.away_runs.unwrap() as i64;
        by_team
            .entry(g.home_team_id)
            .or_default()
            .push((g.date.clone(), hr, ar));
        by_team
            .entry(g.away_team_id)
            .or_default()
            .push((g.date.clone(), ar, hr));
    }
    let mut out = HashMap::new();
    for (team_id, mut rows) in by_team {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let take = rows.len().saturating_sub(window);
        let slice = &rows[take..];
        let games_n = slice.len() as i32;
        if games_n == 0 {
            continue;
        }
        let rs: i64 = slice.iter().map(|(_, rs, _)| rs).sum();
        let ra: i64 = slice.iter().map(|(_, _, ra)| ra).sum();
        out.insert(
            team_id,
            RecentForm {
                games: games_n,
                rs_per_game: rs as f64 / games_n as f64,
                ra_per_game: ra as f64 / games_n as f64,
            },
        );
    }
    out
}

/// Mean of each game's Pythagorean W%. A 22–0 is one observation (~1.000),
/// not +22 runs in a season pile. `recent_pythag` is the same mean over the
/// last `window` games (for the L20 blend).
#[derive(Debug, Clone, Copy)]
pub struct GameVote {
    pub games: i32,
    pub pythag: f64,
    pub recent_games: i32,
    pub recent_pythag: f64,
}

impl GameVote {
    pub fn blended(self, include_recent: bool) -> f64 {
        if include_recent && self.recent_games >= MIN_RECENT_GAMES {
            RECENT_FORM_WEIGHT * self.recent_pythag + (1.0 - RECENT_FORM_WEIGHT) * self.pythag
        } else {
            self.pythag
        }
    }
}

pub fn compute_game_vote_pythag(
    games: &[Game],
    exponent: f64,
    window: usize,
) -> HashMap<i32, GameVote> {
    let mut by_team: HashMap<i32, Vec<(String, f64, f64)>> = HashMap::new();
    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap() as f64;
        let ar = g.away_runs.unwrap() as f64;
        by_team
            .entry(g.home_team_id)
            .or_default()
            .push((g.date.clone(), hr, ar));
        by_team
            .entry(g.away_team_id)
            .or_default()
            .push((g.date.clone(), ar, hr));
    }
    let mean = |slice: &[(String, f64, f64)], exp: f64| -> f64 {
        if slice.is_empty() {
            return 0.5;
        }
        slice
            .iter()
            .map(|(_, rs, ra)| pythag_win_pct(*rs, *ra, exp))
            .sum::<f64>()
            / slice.len() as f64
    };
    let mut out = HashMap::new();
    for (team_id, mut rows) in by_team {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        if rows.is_empty() {
            continue;
        }
        let take = rows.len().saturating_sub(window);
        let recent = &rows[take..];
        out.insert(
            team_id,
            GameVote {
                games: rows.len() as i32,
                pythag: mean(&rows, exponent),
                recent_games: recent.len() as i32,
                recent_pythag: mean(recent, exponent),
            },
        );
    }
    out
}

// One completed meeting between the two teams this season.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct H2HMeeting {
    pub game_pk: i64,
    pub date: String,
    pub home_name: String,
    pub away_name: String,
    pub home_runs: i32,
    pub away_runs: i32,
}

// Season head-to-head record between team A and team B. `a_id`/`b_id` fix which
// side of the wins/runs tallies is which so the frontend can label without guessing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadToHead {
    pub a_id: i32,
    pub b_id: i32,
    pub a_wins: i32,
    pub b_wins: i32,
    pub a_runs: i32,
    pub b_runs: i32,
    pub meetings: Vec<H2HMeeting>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitLine {
    pub games: i32,
    pub wins: i32,
    pub losses: i32,
    pub rs_per_game: f64,
    pub ra_per_game: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSplits {
    pub home: SplitLine,
    pub road: SplitLine,
    // L10 form (last 10 completed games, home + road). None when no games yet.
    pub l10: Option<RecentForm>,
}

// Season series between two teams, computed from completed games already in memory.
pub fn compute_head_to_head(games: &[Game], a_id: i32, b_id: i32) -> HeadToHead {
    let mut h = HeadToHead {
        a_id,
        b_id,
        a_wins: 0,
        b_wins: 0,
        a_runs: 0,
        b_runs: 0,
        meetings: Vec::new(),
    };
    for g in games.iter().filter(|g| g.is_final()) {
        let ids = (g.home_team_id, g.away_team_id);
        if ids != (a_id, b_id) && ids != (b_id, a_id) {
            continue;
        }
        let hr = g.home_runs.unwrap();
        let ar = g.away_runs.unwrap();
        // Map this game's home/away runs onto the A/B tallies.
        let (a_r, b_r) = if g.home_team_id == a_id { (hr, ar) } else { (ar, hr) };
        h.a_runs += a_r;
        h.b_runs += b_r;
        if a_r > b_r {
            h.a_wins += 1;
        } else if b_r > a_r {
            h.b_wins += 1;
        }
        h.meetings.push(H2HMeeting {
            game_pk: g.game_pk,
            date: g.date.clone(),
            home_name: g.home_team_name.clone(),
            away_name: g.away_team_name.clone(),
            home_runs: hr,
            away_runs: ar,
        });
    }
    h.meetings.sort_by(|x, y| x.date.cmp(&y.date));
    h
}

// Home / road / last-10 splits for one team, from completed games in memory.
pub fn compute_splits(games: &[Game], team_id: i32) -> TeamSplits {
    let mut home = SplitAgg::default();
    let mut road = SplitAgg::default();
    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap();
        let ar = g.away_runs.unwrap();
        if g.home_team_id == team_id {
            home.add(hr, ar);
        } else if g.away_team_id == team_id {
            road.add(ar, hr);
        }
    }
    TeamSplits {
        home: home.finish(),
        road: road.finish(),
        l10: compute_recent_form(games, 10).get(&team_id).copied(),
    }
}

pub fn compute_all_splits(games: &[Game]) -> HashMap<i32, TeamSplits> {
    let mut home_a: HashMap<i32, SplitAgg> = HashMap::new();
    let mut road_a: HashMap<i32, SplitAgg> = HashMap::new();
    for g in games.iter().filter(|g| g.is_final()) {
        let hr = g.home_runs.unwrap();
        let ar = g.away_runs.unwrap();
        home_a.entry(g.home_team_id).or_default().add(hr, ar);
        road_a.entry(g.away_team_id).or_default().add(ar, hr);
    }
    let mut ids: Vec<i32> = home_a.keys().copied().chain(road_a.keys().copied()).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut out = HashMap::new();
    for id in ids {
        let home = home_a.remove(&id).unwrap_or_default().finish();
        let road = road_a.remove(&id).unwrap_or_default().finish();
        out.insert(id, TeamSplits { home, road, l10: None });
    }
    out
}

pub(crate) fn stats_with_split(base: &TeamStats, line: SplitLine) -> TeamStats {
    if line.games < MIN_RECENT_GAMES {
        return base.clone();
    }
    let g = 1000i32;
    let mut t = base.clone();
    t.runs_scored = (line.rs_per_game * g as f64).round() as i32;
    t.runs_allowed = (line.ra_per_game * g as f64).round() as i32;
    t.games_played = g;
    t
}

#[derive(Default)]
struct SplitAgg {
    games: i32,
    wins: i32,
    losses: i32,
    rs: i32,
    ra: i32,
}

impl SplitAgg {
    fn add(&mut self, scored: i32, allowed: i32) {
        self.games += 1;
        self.rs += scored;
        self.ra += allowed;
        if scored > allowed {
            self.wins += 1;
        } else if allowed > scored {
            self.losses += 1;
        }
    }
    fn finish(&self) -> SplitLine {
        let g = self.games.max(1) as f64;
        SplitLine {
            games: self.games,
            wins: self.wins,
            losses: self.losses,
            rs_per_game: round_to(self.rs as f64 / g, 2),
            ra_per_game: round_to(self.ra as f64 / g, 2),
        }
    }
}

// Blend a season-level per-game rate with the L20 rate. If the recent sample
// is missing or too small (early season), fall back to the season number.
fn blend(season_per_g: f64, recent: Option<(f64, i32)>, weight: f64) -> f64 {
    match recent {
        Some((rate, n)) if n >= MIN_RECENT_GAMES => weight * rate + (1.0 - weight) * season_per_g,
        _ => season_per_g,
    }
}

fn season_rs_pg(team: &TeamStats) -> f64 {
    if team.games_played > 0 {
        team.runs_scored as f64 / team.games_played as f64
    } else {
        4.5
    }
}

fn season_ra_pg(team: &TeamStats) -> f64 {
    if team.games_played > 0 {
        team.runs_allowed as f64 / team.games_played as f64
    } else {
        4.5
    }
}

// Blend the team's (possibly recent-form-adjusted) RA/G with the starter.
// NextStart: outing ER + remaining innings at team RA/G.
// Era fallback: starter accounts for STARTER_SHARE of RA; rest is bullpen / staff.
// Below MIN_IP_FOR_ADJUSTMENT on the Era path we fall back to team RA.
pub fn apply_pitcher(team_ra_pg: f64, pitcher: Option<PitcherAdj>) -> f64 {
    match pitcher {
        Some(p) if p.source == PitcherSource::NextStart => {
            let rest = p.bullpen_ra.unwrap_or(team_ra_pg);
            let bullpen_share = (1.0 - p.innings_pitched / 9.0).max(0.0);
            p.rate_or_er + bullpen_share * rest
        }
        Some(p) if p.innings_pitched >= MIN_IP_FOR_ADJUSTMENT => {
            let rest = p.bullpen_ra.unwrap_or(team_ra_pg);
            STARTER_SHARE * p.rate_or_er + (1.0 - STARTER_SHARE) * rest
        }
        _ => team_ra_pg,
    }
}

/// Live Predictions path: OS/DS shrink 40% toward 1.0, then Overdisp win %
/// (NB size 2 + 40% run-line shrink) on that score, then HFA.
pub fn estimate_game_with_pitchers(
    home: &TeamStats,
    away: &TeamStats,
    lg_avg_runs: f64,
    home_pitcher: Option<PitcherAdj>,
    away_pitcher: Option<PitcherAdj>,
    home_recent: Option<RecentForm>,
    away_recent: Option<RecentForm>,
    exponent: f64,
    apply_home_field: bool,
) -> Prediction {
    estimate_game_detailed(
        home,
        away,
        lg_avg_runs,
        home_pitcher,
        away_pitcher,
        home_recent,
        away_recent,
        exponent,
        apply_home_field,
        None,
        None,
        false,
        true,
        1.0,
        RATE_SHRINK,
    )
    .0
}

// Same math as estimate_game_with_pitchers, but also returns the intermediate
// values behind the prediction (for the game-detail view). This is the single
// source of truth — estimate_game_with_pitchers is a thin wrapper over it.
#[allow(clippy::too_many_arguments)]
pub fn estimate_game_detailed(
    home: &TeamStats,
    away: &TeamStats,
    lg_avg_runs: f64,
    home_pitcher: Option<PitcherAdj>,
    away_pitcher: Option<PitcherAdj>,
    home_recent: Option<RecentForm>,
    away_recent: Option<RecentForm>,
    exponent: f64,
    apply_home_field: bool,
    home_game_vote: Option<f64>,
    away_game_vote: Option<f64>,
    use_poisson_win: bool,
    use_nb_win: bool,
    park_factor: f64,
    rate_shrink: f64,
) -> (Prediction, MatchupBreakdown) {
    // Season-level RS/G and team RA/G. Each may be blended with the team's L20
    // form (if a RecentForm is provided AND the sample meets MIN_RECENT_GAMES).
    let home_season_rs_pg = season_rs_pg(home);
    let away_season_rs_pg = season_rs_pg(away);
    let home_season_ra_pg = season_ra_pg(home);
    let away_season_ra_pg = season_ra_pg(away);

    let home_rs_pg = blend(
        home_season_rs_pg,
        home_recent.map(|r| (r.rs_per_game, r.games)),
        RECENT_FORM_WEIGHT,
    );
    let away_rs_pg = blend(
        away_season_rs_pg,
        away_recent.map(|r| (r.rs_per_game, r.games)),
        RECENT_FORM_WEIGHT,
    );
    let home_ra_team = blend(
        home_season_ra_pg,
        home_recent.map(|r| (r.ra_per_game, r.games)),
        RECENT_FORM_WEIGHT,
    );
    let away_ra_team = blend(
        away_season_ra_pg,
        away_recent.map(|r| (r.ra_per_game, r.games)),
        RECENT_FORM_WEIGHT,
    );

    // Pitcher adjustment (next-start ER+IP, else season ERA) runs on the
    // possibly L20-blended team RA/G. Order: season → recent form → starter.
    let home_ra_eff = apply_pitcher(home_ra_team, home_pitcher);
    let away_ra_eff = apply_pitcher(away_ra_team, away_pitcher);

    // Rate Pythagorean (pooled RS/G vs effective RA). Game-vote mode replaces
    // this with the mean of per-game Pythags, then applies the pitcher as a
    // log-odds shift so a 22–0 is one game, not +22 runs in the pile.
    let home_pyt_rate = pythag_win_pct(home_rs_pg, home_ra_eff, exponent);
    let away_pyt_rate = pythag_win_pct(away_rs_pg, away_ra_eff, exponent);
    let home_pyt = match home_game_vote {
        Some(v) => {
            let pre = pythag_win_pct(home_rs_pg, home_ra_team, exponent);
            shift_log_odds(v, logit(home_pyt_rate) - logit(pre))
        }
        None => home_pyt_rate,
    };
    let away_pyt = match away_game_vote {
        Some(v) => {
            let pre = pythag_win_pct(away_rs_pg, away_ra_team, exponent);
            shift_log_odds(v, logit(away_pyt_rate) - logit(pre))
        }
        None => away_pyt_rate,
    };

    // Predicted runs: derive OS from the (possibly blended) RS/G and DS from
    // the effective RA. When no recent form / pitcher is supplied this collapses
    // back to the original season-level OS · DS · lg_avg_runs.
    let s = rate_shrink.clamp(0.0, 1.0);
    let home_os_eff = shrink_to_one(home_rs_pg / lg_avg_runs, s);
    let away_os_eff = shrink_to_one(away_rs_pg / lg_avg_runs, s);
    let home_ds_eff = shrink_to_one(home_ra_eff / lg_avg_runs, s);
    let away_ds_eff = shrink_to_one(away_ra_eff / lg_avg_runs, s);
    let pf = if park_factor > 0.0 { park_factor } else { 1.0 };
    let home_pred = home_os_eff * away_ds_eff * lg_avg_runs * pf;
    let away_pred = away_os_eff * home_ds_eff * lg_avg_runs * pf;
    let total = home_pred + away_pred;

    // Clamp unconditionally: log5 can return exactly 0 or 1 for degenerate
    // inputs (e.g. a team with RA=0 over a small sample), which would make
    // prob_to_american_odds hit its invalid-probability guard and return 0.
    let neutral_home_win = if use_nb_win {
        nb_home_win(
            shrink_run_line(home_pred, lg_avg_runs),
            shrink_run_line(away_pred, lg_avg_runs),
        )
    } else if use_poisson_win {
        poisson_home_win(home_pred, away_pred)
    } else {
        log5(home_pyt, away_pyt)
    }
    .clamp(1e-9, 1.0 - 1e-9);
    let home_win = if apply_home_field {
        shift_log_odds(neutral_home_win, HOME_FIELD_LOG_ODDS)
    } else {
        neutral_home_win
    };
    let away_win = 1.0 - home_win;

    let pred = Prediction {
        home_win_prob: round_to(home_win, 4),
        away_win_prob: round_to(away_win, 4),
        home_fair_odds: prob_to_american_odds(home_win),
        away_fair_odds: prob_to_american_odds(away_win),
        home_pred_runs: round_to(home_pred, 2),
        away_pred_runs: round_to(away_pred, 2),
        total_runs: round_to(total, 2),
    };

    let side = |season_rs: f64,
                season_ra: f64,
                recent: Option<RecentForm>,
                blended_rs: f64,
                blended_ra: f64,
                pitcher: Option<PitcherAdj>,
                ra_eff: f64,
                pyt: f64,
                os_eff: f64,
                ds_eff: f64,
                pred_runs: f64|
     -> SideBreakdown {
        let recent_applied = matches!(recent, Some(r) if r.games >= MIN_RECENT_GAMES);
        let pitcher_applied = pitcher.map(|p| p.is_applied()).unwrap_or(false);
        let (pitcher_source, era_val, exp_runs, exp_ip) = match pitcher {
            Some(p) if p.source == PitcherSource::NextStart => {
                ("nextStart", 0.0, p.rate_or_er, p.innings_pitched)
            }
            Some(p) => ("era", p.rate_or_er, 0.0, 0.0),
            None => ("none", 0.0, 0.0, 0.0),
        };
        SideBreakdown {
            season_rs_per_game: round_to(season_rs, 2),
            season_ra_per_game: round_to(season_ra, 2),
            recent_present: recent.is_some(),
            recent_games: recent.map(|r| r.games).unwrap_or(0),
            recent_rs_per_game: round_to(recent.map(|r| r.rs_per_game).unwrap_or(0.0), 2),
            recent_ra_per_game: round_to(recent.map(|r| r.ra_per_game).unwrap_or(0.0), 2),
            recent_applied,
            blended_rs_per_game: round_to(blended_rs, 2),
            blended_ra_per_game: round_to(blended_ra, 2),
            pitcher_present: pitcher.is_some(),
            pitcher_era: round_to(era_val, 2),
            pitcher_ip: round_to(pitcher.map(|p| p.innings_pitched).unwrap_or(0.0), 1),
            pitcher_applied,
            pitcher_source,
            pitcher_expected_runs: round_to(exp_runs, 2),
            pitcher_expected_innings: round_to(exp_ip, 1),
            effective_ra_per_game: round_to(ra_eff, 2),
            pythag_win_pct: round_to(pyt, 4),
            os_eff: round_to(os_eff, 3),
            ds_eff: round_to(ds_eff, 3),
            pred_runs: round_to(pred_runs, 2),
        }
    };

    let breakdown = MatchupBreakdown {
        home: side(
            home_season_rs_pg, home_season_ra_pg, home_recent, home_rs_pg, home_ra_team,
            home_pitcher, home_ra_eff, home_pyt, home_os_eff, home_ds_eff, home_pred,
        ),
        away: side(
            away_season_rs_pg, away_season_ra_pg, away_recent, away_rs_pg, away_ra_team,
            away_pitcher, away_ra_eff, away_pyt, away_os_eff, away_ds_eff, away_pred,
        ),
        exponent: round_to(exponent, 4),
        league_avg_runs: round_to(lg_avg_runs, 3),
        neutral_home_win: round_to(neutral_home_win, 4),
        home_field_applied: apply_home_field,
        home_field_delta: if apply_home_field { HOME_FIELD_LOG_ODDS } else { 0.0 },
        final_home_win: round_to(home_win, 4),
        final_away_win: round_to(away_win, 4),
    };

    (pred, breakdown)
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
    use crate::mlb_api::GameStatus;

    // A completed game for split/H2H tests.
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

    #[test]
    fn head_to_head_tallies_by_side() {
        let games = vec![
            fin(1, "2026-04-01", 10, "A", 5, 20, "B", 3), // A home, A wins
            fin(2, "2026-04-02", 20, "B", 7, 10, "A", 2), // B home, B wins
            fin(3, "2026-04-03", 10, "A", 1, 20, "B", 4), // A home, B wins
            fin(4, "2026-04-04", 30, "C", 9, 10, "A", 0), // unrelated
        ];
        let h = compute_head_to_head(&games, 10, 20);
        assert_eq!(h.a_wins, 1);
        assert_eq!(h.b_wins, 2);
        assert_eq!(h.a_runs, 5 + 2 + 1);
        assert_eq!(h.b_runs, 3 + 7 + 4);
        assert_eq!(h.meetings.len(), 3);
        assert_eq!(h.meetings[0].date, "2026-04-01"); // sorted
    }

    #[test]
    fn splits_separate_home_and_road() {
        let games = vec![
            fin(1, "2026-04-01", 10, "A", 6, 20, "B", 2), // A home win, 6/2
            fin(2, "2026-04-02", 10, "A", 1, 30, "C", 5), // A home loss, 1/5
            fin(3, "2026-04-03", 40, "D", 3, 10, "A", 8), // A road win, 8/3
        ];
        let s = compute_splits(&games, 10);
        assert_eq!(s.home.games, 2);
        assert_eq!(s.home.wins, 1);
        assert_eq!(s.home.losses, 1);
        assert!((s.home.rs_per_game - 3.5).abs() < 1e-9); // (6+1)/2
        assert!((s.home.ra_per_game - 3.5).abs() < 1e-9); // (2+5)/2
        assert_eq!(s.road.games, 1);
        assert_eq!(s.road.wins, 1);
        assert!((s.road.rs_per_game - 8.0).abs() < 1e-9);
    }

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

    #[test]
    fn home_field_shift_at_coin_flip() {
        // 50% → ~54% with the historical log-odds shift
        let adj = shift_log_odds(0.5, HOME_FIELD_LOG_ODDS);
        assert!((adj - 0.54).abs() < 0.001);
    }

    #[test]
    fn home_field_shift_shrinks_at_extremes() {
        // A 90% favorite should bump by less than 4 percentage points
        let adj = shift_log_odds(0.9, HOME_FIELD_LOG_ODDS);
        assert!(adj > 0.9 && adj < 0.92);
        // A 10% dog likewise — bumped, but not by 4 absolute points
        let adj_low = shift_log_odds(0.1, HOME_FIELD_LOG_ODDS);
        assert!(adj_low > 0.1 && adj_low < 0.12);
    }

    #[test]
    fn recent_form_blend_is_weighted_combination() {
        // 60/40 season-heavy: season 4.0, recent 6.0 → 0.6*4 + 0.4*6 = 4.8
        let b = blend(4.0, Some((6.0, 20)), RECENT_FORM_WEIGHT);
        assert!((b - 4.8).abs() < 1e-9);
    }

    #[test]
    fn recent_form_falls_back_below_min_games() {
        // Sample too small → just return the season value
        let b = blend(4.0, Some((6.0, 5)), RECENT_FORM_WEIGHT);
        assert!((b - 4.0).abs() < 1e-9);
        // None → also fall back
        let b2 = blend(4.0, None, RECENT_FORM_WEIGHT);
        assert!((b2 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn recent_form_shifts_prediction_toward_hot_team() {
        let home = TeamStats {
            team_id: 1, team: "H".into(), runs_scored: 400, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.5, os: 1.0, ds: 1.0,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let away = TeamStats {
            team_id: 2, team: "A".into(), runs_scored: 400, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.5, os: 1.0, ds: 1.0,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        // No recent form → 50/50.
        let neutral = estimate_game_with_pitchers(&home, &away, 4.0, None, None, None, None, 2.0, false);
        assert!((neutral.home_win_prob - 0.5).abs() < 1e-9);

        // Home is scorching, away is cold → home should be a clear favorite.
        let home_hot = Some(RecentForm { games: 20, rs_per_game: 6.0, ra_per_game: 3.0 });
        let away_cold = Some(RecentForm { games: 20, rs_per_game: 3.0, ra_per_game: 6.0 });
        let hot = estimate_game_with_pitchers(&home, &away, 4.0, None, None, home_hot, away_cold, 2.0, false);
        assert!(hot.home_win_prob > 0.55, "expected home favorite after shrink, got {}", hot.home_win_prob);
        // Live path is Overdisp, not log5 — equal run lines still 50/50; a mismatch
        // is a favorite but not a lock.
        let (log5_pred, _) = estimate_game_detailed(
            &home, &away, 4.0, None, None, home_hot, away_cold, 2.0, false, None, None, false, false, 1.0, 0.0,
        );
        assert!(
            (hot.home_win_prob - 0.5).abs() < (log5_pred.home_win_prob - 0.5).abs(),
            "live Overdisp should be softer than log5: nb={} log5={}",
            hot.home_win_prob, log5_pred.home_win_prob
        );
    }

    #[test]
    fn detailed_breakdown_matches_prediction() {
        // The breakdown's final win probs and predicted runs must equal the
        // headline Prediction — otherwise the detail view would silently diverge.
        let home = TeamStats {
            team_id: 1, team: "H".into(), runs_scored: 480, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.6, os: 1.1, ds: 0.9,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let away = TeamStats {
            team_id: 2, team: "A".into(), runs_scored: 420, runs_allowed: 450,
            games_played: 100, pythag_win_pct: 0.45, os: 0.95, ds: 1.05,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let hp = Some(PitcherAdj::from_era(3.10, 120.0));
        let ap = Some(PitcherAdj::from_era(4.80, 90.0));
        let hr = Some(RecentForm { games: 20, rs_per_game: 5.2, ra_per_game: 3.8 });
        let ar = Some(RecentForm { games: 20, rs_per_game: 3.9, ra_per_game: 4.6 });

        let (pred, bd) =
            estimate_game_detailed(&home, &away, 4.5, hp, ap, hr, ar, 1.83, true, None, None, false, false, 1.0, 0.0);

        assert_eq!(bd.final_home_win, pred.home_win_prob);
        assert_eq!(bd.final_away_win, pred.away_win_prob);
        assert_eq!(bd.home.pred_runs, pred.home_pred_runs);
        assert_eq!(bd.away.pred_runs, pred.away_pred_runs);
        assert!(bd.home.pitcher_applied && bd.home.recent_applied);
        assert!(bd.home_field_applied && bd.home_field_delta > 0.0);
    }

    // ── Exponent fitter ──────────────────────────────────────────────────

    #[test]
    fn optimize_exponent_recovers_reasonable_value() {
        // Generate a synthetic season where each team's wins follow a known
        // Pythagorean relationship with a target exponent of ~1.83. The fitter
        // should recover a value near 1.83, not pinned to the 0.5/5.0 boundary.
        let target_exp = 1.83;
        let teams = [
            (1, "Elite", 800, 650),
            (2, "Good",  750, 700),
            (3, "Above", 720, 700),
            (4, "Below", 700, 720),
            (5, "Poor",  700, 750),
            (6, "Bad",   650, 800),
        ];
        // For each team, compute its Pythag win% at target_exp, then generate
        // 162 games of W/L whose count matches that win%.
        let mut games = vec![];
        let mut pk = 1i64;
        for i in 0..teams.len() {
            for j in (i + 1)..teams.len() {
                let (hi, hn, hrs, hra) = teams[i];
                let (ai, an, ars, ara) = teams[j];
                let ph = pythag_win_pct(hrs as f64, hra as f64, target_exp);
                // 10-game series; home team wins round(10 * log5(ph, pa)) games.
                let pa = pythag_win_pct(ars as f64, ara as f64, target_exp);
                let p_matchup = log5(ph, pa);
                let h_wins = (10.0 * p_matchup).round() as i32;
                for g in 0..10 {
                    let (hr, ar) = if g < h_wins { (5, 3) } else { (3, 5) };
                    games.push(fin(pk, "2026-04-01", hi, hn, hr, ai, an, ar));
                    pk += 1;
                }
            }
        }
        let exp = optimize_exponent(&games);
        assert!(exp > 0.5 && exp < 5.0, "exponent pinned to boundary: {exp}");
    }

    #[test]
    fn optimize_exponent_falls_back_below_two_teams() {
        // A single team's games can't fit a league-wide exponent — fall back to 2.0.
        let games = vec![fin(1, "2026-04-01", 1, "A", 5, 1, "A", 3)];
        let exp = optimize_exponent(&games);
        assert!((exp - 2.0).abs() < 1e-9);
        // Empty input too.
        assert!((optimize_exponent(&[]) - 2.0).abs() < 1e-9);
    }

    // ── Pitcher blend ───────────────────────────────────────────────────

    #[test]
    fn apply_pitcher_blends_era_with_team_ra() {
        // 0.6 * 3.0 + 0.4 * 4.5 = 1.8 + 1.8 = 3.6
        let adj = Some(PitcherAdj::from_era(3.0, 100.0));
        let eff = apply_pitcher(4.5, adj);
        assert!((eff - 3.6).abs() < 1e-9);
    }

    #[test]
    fn apply_pitcher_boundary_at_min_ip() {
        // At exactly MIN_IP_FOR_ADJUSTMENT (20.0) the adjustment applies…
        let at = Some(PitcherAdj::from_era(3.0, 20.0));
        assert!((apply_pitcher(4.5, at) - 3.6).abs() < 1e-9);
        // …just below it (19.9) it falls back to the team RA.
        let below = Some(PitcherAdj::from_era(3.0, 19.9));
        assert!((apply_pitcher(4.5, below) - 4.5).abs() < 1e-9);
        // None also falls back.
        assert!((apply_pitcher(4.5, None) - 4.5).abs() < 1e-9);
    }

    #[test]
    fn apply_pitcher_next_start_outing_blend() {
        // ER 2.3, IP 5.3, team RA/G 4.5 → 2.3 + (1 - 5.3/9)*4.5 = 4.15
        let adj = Some(PitcherAdj::from_next_start(2.3, 5.3));
        let eff = apply_pitcher(4.5, adj);
        assert!((eff - 4.15).abs() < 1e-9);
        // IP ≥ 9 → no bullpen remainder.
        let full = Some(PitcherAdj::from_next_start(3.1, 9.0));
        assert!((apply_pitcher(4.5, full) - 3.1).abs() < 1e-9);
        // Next-start applies even with a tiny IP sample (PNS already shrinks).
        let short = Some(PitcherAdj::from_next_start(1.2, 4.0));
        let expected = 1.2 + (1.0 - 4.0 / 9.0) * 4.5;
        assert!((apply_pitcher(4.5, short) - expected).abs() < 1e-9);
    }

    #[test]
    fn apply_pitcher_uses_bullpen_rate_for_leftover() {
        let mut adj = PitcherAdj::from_era(3.0, 100.0);
        adj.bullpen_ra = Some(6.0);
        // 0.6 * 3.0 + 0.4 * 6.0 = 4.2 (not 0.4 * team 4.5)
        assert!((apply_pitcher(4.5, Some(adj)) - 4.2).abs() < 1e-9);
        let mut ns = PitcherAdj::from_next_start(2.3, 5.3);
        ns.bullpen_ra = Some(6.0);
        let expected = 2.3 + (1.0 - 5.3 / 9.0) * 6.0;
        assert!((apply_pitcher(4.5, Some(ns)) - expected).abs() < 1e-9);
        assert!((apply_pitcher(4.5, None) - 4.5).abs() < 1e-9);
    }

    // ── Recent-form window count ────────────────────────────────────────

    #[test]
    fn compute_recent_form_takes_exactly_window_games() {
        // 25 completed games → the L20 window should contain exactly 20.
        let mut games = vec![];
        for i in 0..25 {
            games.push(fin(i as i64 + 1, &format!("2026-04-{:02}", i + 1), 10, "A", 4, 20, "B", 3));
        }
        let form = compute_recent_form(&games, RECENT_FORM_WINDOW);
        let r = form.get(&10).expect("team 10 should have recent form");
        assert_eq!(r.games, 20, "expected exactly 20 games in the window");
    }

    // ── Numerical edge cases ────────────────────────────────────────────

    #[test]
    fn game_vote_downweights_blowout() {
        // 10 games of 5–4 plus a 22–0. Pooled Pythagorean treats +22 as a pile of
        // runs; game-vote treats it as one ~1.000 game.
        let mut games = Vec::new();
        for i in 0..10 {
            games.push(fin(i + 1, "2026-04-01", 1, "MIL", 5, 2, "OPP", 4));
        }
        games.push(fin(99, "2026-04-02", 1, "MIL", 22, 2, "OPP", 0));
        let vote = compute_game_vote_pythag(&games, 2.0, 20);
        let (stats, _) = compute_team_stats(&games, 2.0);
        let pooled = stats.iter().find(|t| t.team_id == 1).unwrap().pythag_win_pct;
        let v = vote.get(&1).unwrap().pythag;
        assert!(v < pooled - 0.05, "game-vote {v} should sit well below pooled {pooled}");
        assert!((v - 0.645).abs() < 0.01, "expected ~0.645, got {v}");
    }

    #[test]
    fn game_vote_does_not_change_predicted_runs() {
        let home = TeamStats {
            team_id: 1, team: "H".into(), runs_scored: 480, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.6, os: 1.1, ds: 0.9,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let away = TeamStats {
            team_id: 2, team: "A".into(), runs_scored: 420, runs_allowed: 450,
            games_played: 100, pythag_win_pct: 0.45, os: 0.95, ds: 1.05,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let (a, _) = estimate_game_detailed(&home, &away, 4.5, None, None, None, None, 1.83, false, None, None, false, false, 1.0, 0.0);
        let (b, _) = estimate_game_detailed(&home, &away, 4.5, None, None, None, None, 1.83, false, Some(0.55), Some(0.48), false, false, 1.0, 0.0);
        assert!((a.home_pred_runs - b.home_pred_runs).abs() < 1e-9);
        assert!((a.away_pred_runs - b.away_pred_runs).abs() < 1e-9);
        assert!(a.home_win_prob != b.home_win_prob);
    }

    #[test]
    fn pythag_handles_zero_runs() {
        // RS=0, RA=0 → the 0^x/(0^x+0^x) degenerate path must not panic or NaN.
        let p = pythag_win_pct(0.0, 0.0, 1.83);
        assert!(p.is_finite(), "pythag(0,0) must be finite, got {p}");
        // RS=0, RA>0 → team never wins.
        assert!((pythag_win_pct(0.0, 700.0, 2.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn poisson_home_win_equal_means_is_coin_flip() {
        let p = poisson_home_win(4.5, 4.5);
        assert!((p - 0.5).abs() < 1e-9, "got {p}");
    }

    #[test]
    fn poisson_home_win_favors_higher_mean() {
        let p = poisson_home_win(6.0, 3.0);
        assert!(p > 0.75 && p < 0.95, "6 vs 3 should be a clear but not certain favorite, got {p}");
        assert!((poisson_home_win(3.0, 6.0) - (1.0 - p)).abs() < 1e-9);
    }

    #[test]
    fn nb_home_win_is_softer_than_poisson() {
        assert!((nb_home_win(4.5, 4.5) - 0.5).abs() < 1e-9);
        let p_nb = nb_home_win(6.0, 3.0);
        let p_po = poisson_home_win(6.0, 3.0);
        assert!(p_nb > 0.5, "6 vs 3 is still a favorite, got {p_nb}");
        assert!(p_nb < p_po, "NB should be less sure than Poisson: nb={p_nb} pois={p_po}");
    }

    #[test]
    fn park_run_factor_known_venues() {
        assert!((park_run_factor(Some(19)) - 1.25).abs() < 1e-9); // Coors
        assert!((park_run_factor(Some(680)) - 0.83).abs() < 1e-9); // T-Mobile
        assert!((park_run_factor(None) - 1.0).abs() < 1e-9);
        assert!((park_run_factor(Some(99999)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn shrink_to_one_is_convex() {
        assert!((shrink_to_one(1.333, 0.4) - 1.2).abs() < 0.001);
        assert!((shrink_to_one(1.0, 0.4) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn shrink_run_line_moves_toward_league_avg() {
        assert!((shrink_run_line(6.0, 4.5) - 5.4).abs() < 1e-9);
        assert!((shrink_run_line(3.0, 4.5) - 3.6).abs() < 1e-9);
        let raw = nb_home_win(6.0, 3.0);
        let shrunk = nb_home_win(shrink_run_line(6.0, 4.5), shrink_run_line(3.0, 4.5));
        assert!(shrunk > 0.5);
        assert!(
            (shrunk - 0.5).abs() < (raw - 0.5).abs(),
            "shrunk line should be closer to 50%: raw={raw} shrunk={shrunk}"
        );
    }

    #[test]
    fn log5_handles_degenerate_inputs() {
        // log5 must not panic or produce NaN at the 0/1 extremes.
        assert!(log5(0.0, 0.5).is_finite());
        assert!(log5(1.0, 0.5).is_finite());
        assert!((log5(0.5, 0.5) - 0.5).abs() < 1e-9);
    }

    // ── Spec invariants ─────────────────────────────────────────────────

    #[test]
    fn home_field_does_not_affect_predicted_runs() {
        // Spec step 6: HFA shifts win probability only — predicted runs are
        // OS×DS×league-avg and must be identical whether HFA is on or off.
        let home = TeamStats {
            team_id: 1, team: "H".into(), runs_scored: 480, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.6, os: 1.1, ds: 0.9,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let away = TeamStats {
            team_id: 2, team: "A".into(), runs_scored: 420, runs_allowed: 450,
            games_played: 100, pythag_win_pct: 0.45, os: 0.95, ds: 1.05,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let with_hfa =
            estimate_game_with_pitchers(&home, &away, 4.5, None, None, None, None, 1.83, true);
        let without_hfa =
            estimate_game_with_pitchers(&home, &away, 4.5, None, None, None, None, 1.83, false);
        assert!((with_hfa.home_pred_runs - without_hfa.home_pred_runs).abs() < 1e-9);
        assert!((with_hfa.away_pred_runs - without_hfa.away_pred_runs).abs() < 1e-9);
        // But win prob DOES differ (home gets the bump).
        assert!(with_hfa.home_win_prob > without_hfa.home_win_prob);
    }

    #[test]
    fn pitcher_operates_on_recency_blended_ra() {
        // Spec: the pitcher blend operates on the L20-blended RA, not the raw
        // season RA. With recent form moving RA one way and the pitcher ERA
        // another, effective_ra must equal 0.6*era + 0.4*blended_ra.
        let home = TeamStats {
            team_id: 1, team: "H".into(), runs_scored: 480, runs_allowed: 400,
            games_played: 100, pythag_win_pct: 0.6, os: 1.1, ds: 0.9,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        let away = TeamStats {
            team_id: 2, team: "A".into(), runs_scored: 420, runs_allowed: 450,
            games_played: 100, pythag_win_pct: 0.45, os: 0.95, ds: 1.05,
            recent_games: None, recent_rs_per_game: None, recent_ra_per_game: None,
        };
        // Season RA/G for home = 400/100 = 4.0. Recent RA/G = 3.0 → blended =
        // 0.6*4.0 + 0.4*3.0 = 3.6. Pitcher ERA 2.0 → eff = 0.6*2.0 + 0.4*3.6 = 2.64.
        // If the pitcher wrongly operated on season RA, we'd get 0.6*2.0+0.4*4.0 = 2.8.
        let hr = Some(RecentForm { games: 20, rs_per_game: 5.0, ra_per_game: 3.0 });
        let hp = Some(PitcherAdj::from_era(2.0, 100.0));
        let (_, bd) = estimate_game_detailed(&home, &away, 4.5, hp, None, hr, None, 1.83, false, None, None, false, false, 1.0, 0.0);
        assert!((bd.home.effective_ra_per_game - 2.64).abs() < 1e-6,
            "expected 2.64 (pitcher on blended RA), got {}", bd.home.effective_ra_per_game);
    }
}
