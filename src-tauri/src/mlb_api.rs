// HTTP client for the MLB Stats API (https://statsapi.mlb.com).
// Mirrors what baseballr::mlb_schedule() wraps in R, but pure Rust.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SCHEDULE_BASE: &str = "https://statsapi.mlb.com/api/v1/schedule";
const USER_AGENT: &str = "mlb-pe-tauri/0.1 (github.com/adamwickwire/mlb-pe)";

#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub date: String, // "YYYY-MM-DD"
    pub status: GameStatus,
    pub series_description: Option<String>,
    pub home_team_id: i32,
    pub home_team_name: String,
    pub home_runs: Option<i32>,
    pub away_team_id: i32,
    pub away_team_name: String,
    pub away_runs: Option<i32>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Preview,
    Live,
    Final,
    Other,
}

impl Game {
    pub fn is_final(&self) -> bool {
        self.status == GameStatus::Final
            && self.home_runs.is_some()
            && self.away_runs.is_some()
    }

    pub fn is_regular_season(&self) -> bool {
        match &self.series_description {
            Some(s) => s == "Regular Season",
            None => false,
        }
    }
}

pub async fn fetch_schedule(season: i32) -> Result<Vec<Game>> {
    let url = format!(
        "{}?sportId=1&season={}&gameType=R",
        SCHEDULE_BASE, season
    );
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;
    let resp: ApiScheduleResponse = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {} failed", url))?
        .error_for_status()
        .context("non-2xx response from MLB API")?
        .json()
        .await
        .context("failed to deserialize MLB schedule")?;
    Ok(normalize(resp))
}

fn normalize(resp: ApiScheduleResponse) -> Vec<Game> {
    let mut out = Vec::with_capacity(2500);
    for day in resp.dates {
        let date = day.date;
        for g in day.games {
            let status = match g.status.abstract_game_state.as_str() {
                "Final" => GameStatus::Final,
                "Live" => GameStatus::Live,
                "Preview" => GameStatus::Preview,
                _ => GameStatus::Other,
            };
            // Use officialDate when present (handles doubleheaders / late starts), else the day's date.
            let game_date = g.official_date.unwrap_or_else(|| date.clone());
            out.push(Game {
                date: game_date,
                status,
                series_description: g.series_description,
                home_team_id: g.teams.home.team.id,
                home_team_name: g.teams.home.team.name,
                home_runs: g.teams.home.score,
                away_team_id: g.teams.away.team.id,
                away_team_name: g.teams.away.team.name,
                away_runs: g.teams.away.score,
            });
        }
    }
    out.retain(|g| g.is_regular_season());
    out
}

// ---- raw API DTOs ----

#[derive(Debug, Deserialize)]
struct ApiScheduleResponse {
    #[serde(default)]
    dates: Vec<ApiScheduleDate>,
}

#[derive(Debug, Deserialize)]
struct ApiScheduleDate {
    date: String,
    #[serde(default)]
    games: Vec<ApiGame>,
}

#[derive(Debug, Deserialize)]
struct ApiGame {
    #[serde(rename = "officialDate")]
    official_date: Option<String>,
    status: ApiStatus,
    teams: ApiTeams,
    #[serde(rename = "seriesDescription")]
    series_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiStatus {
    #[serde(rename = "abstractGameState")]
    abstract_game_state: String,
}

#[derive(Debug, Deserialize)]
struct ApiTeams {
    home: ApiTeamSide,
    away: ApiTeamSide,
}

#[derive(Debug, Deserialize)]
struct ApiTeamSide {
    score: Option<i32>,
    team: ApiTeam,
}

#[derive(Debug, Deserialize)]
struct ApiTeam {
    id: i32,
    name: String,
}
