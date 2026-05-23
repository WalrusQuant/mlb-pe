// HTTP client for the MLB Stats API (https://statsapi.mlb.com).
// Mirrors what baseballr::mlb_schedule() wraps in R, but pure Rust.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SCHEDULE_BASE: &str = "https://statsapi.mlb.com/api/v1/schedule";
const PEOPLE_BASE: &str = "https://statsapi.mlb.com/api/v1/people";
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
    pub home_pitcher_id: Option<i32>,
    pub home_pitcher_name: Option<String>,
    pub away_pitcher_id: Option<i32>,
    pub away_pitcher_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Preview,
    Live,
    Final,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct PitcherStats {
    pub id: i32,
    pub name: String,
    pub era: f64,           // earned-run average
    pub innings_pitched: f64, // decimal innings (e.g. 35.667 for "35.2")
    pub games_started: i32,
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
        "{}?sportId=1&season={}&gameType=R&hydrate=probablePitcher",
        SCHEDULE_BASE, season
    );
    let client = http_client()?;
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

// Fetch season pitching stats for a set of pitcher IDs. Returns a map keyed by pitcher_id.
// Pitchers with no pitching stats for the season (rare — late call-ups etc.) are omitted.
pub async fn fetch_pitcher_stats(
    season: i32,
    ids: &[i32],
) -> Result<HashMap<i32, PitcherStats>> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let client = http_client()?;
    let id_csv = ids
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let url = format!(
        "{}?personIds={}&hydrate=stats(group=[pitching],type=[season],season={})",
        PEOPLE_BASE, id_csv, season
    );
    let resp: ApiPeopleResponse = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {} failed", url))?
        .error_for_status()
        .context("non-2xx response from MLB people API")?
        .json()
        .await
        .context("failed to deserialize MLB people response")?;

    for p in resp.people.into_iter() {
        let id = p.id;
        let name = p.full_name;
        for s in p.stats.unwrap_or_default() {
            if s.group.display_name != "pitching" {
                continue;
            }
            for split in s.splits {
                let st = split.stat;
                let era = st.era.as_deref().and_then(|v| v.parse::<f64>().ok());
                let ip = parse_innings(st.innings_pitched.as_deref());
                let gs = st.games_started.unwrap_or(0);
                if let (Some(era), Some(ip)) = (era, ip) {
                    out.insert(
                        id,
                        PitcherStats {
                            id,
                            name: name.clone(),
                            era,
                            innings_pitched: ip,
                            games_started: gs,
                        },
                    );
                }
            }
        }
    }
    Ok(out)
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")
}

// Baseball notation: "35.2" means 35 + 2/3 innings (NOT 35.2 decimal).
// The fractional part is always 0, 1, or 2 thirds-of-an-inning. Anything else
// is malformed input — return None rather than silently producing a wrong value.
fn parse_innings(s: Option<&str>) -> Option<f64> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    let (whole_s, frac_s) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, "0"),
    };
    let whole: f64 = whole_s.parse().ok()?;
    let thirds: f64 = match frac_s {
        "0" | "" => 0.0,
        "1" => 1.0 / 3.0,
        "2" => 2.0 / 3.0,
        _ => return None,
    };
    Some(whole + thirds)
}

fn normalize(resp: ApiScheduleResponse) -> Vec<Game> {
    let mut out = Vec::with_capacity(2500);
    for day in resp.dates {
        let date = day.date;
        for g in day.games {
            // The schedule API keeps placeholder records for postponed/cancelled games AND
            // the rescheduled record (same gamePk, different dates[] entry). Drop the
            // placeholder — otherwise the rescheduled date shows the matchup twice.
            if matches!(g.status.detailed_state.as_str(), "Postponed" | "Cancelled") {
                continue;
            }
            let status = match g.status.abstract_game_state.as_str() {
                "Final" => GameStatus::Final,
                "Live" => GameStatus::Live,
                "Preview" => GameStatus::Preview,
                _ => GameStatus::Other,
            };
            // Use officialDate when present (handles doubleheaders / late starts), else the day's date.
            let game_date = g.official_date.unwrap_or_else(|| date.clone());
            let (home_pid, home_pname) = g
                .teams
                .home
                .probable_pitcher
                .as_ref()
                .map(|p| (Some(p.id), Some(p.full_name.clone())))
                .unwrap_or((None, None));
            let (away_pid, away_pname) = g
                .teams
                .away
                .probable_pitcher
                .as_ref()
                .map(|p| (Some(p.id), Some(p.full_name.clone())))
                .unwrap_or((None, None));
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
                home_pitcher_id: home_pid,
                home_pitcher_name: home_pname,
                away_pitcher_id: away_pid,
                away_pitcher_name: away_pname,
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
    #[serde(rename = "detailedState", default)]
    detailed_state: String,
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
    #[serde(rename = "probablePitcher")]
    probable_pitcher: Option<ApiPitcherRef>,
}

#[derive(Debug, Deserialize)]
struct ApiTeam {
    id: i32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ApiPitcherRef {
    id: i32,
    #[serde(rename = "fullName")]
    full_name: String,
}

// ---- people / pitcher stats DTOs ----

#[derive(Debug, Deserialize)]
struct ApiPeopleResponse {
    #[serde(default)]
    people: Vec<ApiPerson>,
}

#[derive(Debug, Deserialize)]
struct ApiPerson {
    id: i32,
    #[serde(rename = "fullName")]
    full_name: String,
    stats: Option<Vec<ApiStatsGroup>>,
}

#[derive(Debug, Deserialize)]
struct ApiStatsGroup {
    group: ApiStatGroupName,
    #[serde(default)]
    splits: Vec<ApiStatsSplit>,
}

#[derive(Debug, Deserialize)]
struct ApiStatGroupName {
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct ApiStatsSplit {
    stat: ApiPitchingStat,
}

#[derive(Debug, Deserialize)]
struct ApiPitchingStat {
    #[serde(default)]
    era: Option<String>,
    #[serde(rename = "inningsPitched", default)]
    innings_pitched: Option<String>,
    #[serde(rename = "gamesStarted", default)]
    games_started: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_innings_baseball_notation() {
        assert!((parse_innings(Some("35.2")).unwrap() - 35.6667).abs() < 0.001);
        assert!((parse_innings(Some("12.0")).unwrap() - 12.0).abs() < 1e-6);
        assert!((parse_innings(Some("8.1")).unwrap() - 8.3333).abs() < 0.001);
        assert!((parse_innings(Some("0")).unwrap() - 0.0).abs() < 1e-6);
        assert_eq!(parse_innings(None), None);
        assert_eq!(parse_innings(Some("")), None);
    }

    #[test]
    fn parse_innings_rejects_malformed() {
        // The API only ever produces .0, .1, .2 — anything else is garbage.
        assert_eq!(parse_innings(Some("7.30")), None);
        assert_eq!(parse_innings(Some("7.5")), None);
        assert_eq!(parse_innings(Some("abc")), None);
        assert_eq!(parse_innings(Some("7.x")), None);
    }
}
