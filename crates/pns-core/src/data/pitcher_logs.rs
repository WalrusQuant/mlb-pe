use super::types::PitcherStartLog;
use crate::{PnsError, Result};
use chrono::NaiveDate;
use serde::Deserialize;

const NORMALIZED_FIXTURE: &str = include_str!("../../../../data/fixtures/pitcher_starts.json");

#[derive(Deserialize)]
struct StatsResponse {
    #[serde(default)]
    stats: Vec<StatsGroup>,
}
#[derive(Deserialize)]
struct StatsGroup {
    #[serde(default)]
    splits: Vec<Split>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Split {
    date: String,
    stat: PitchingStat,
    team: Entity,
    player: Player,
    opponent: Entity,
    game: Game,
    is_home: bool,
}
#[derive(Deserialize)]
struct Entity {
    id: u32,
    #[serde(default)]
    name: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Player {
    id: u32,
    full_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    game_pk: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PitchingStat {
    #[serde(default)]
    games_started: u16,
    innings_pitched: String,
    #[serde(default)]
    strike_outs: u16,
    #[serde(default)]
    base_on_balls: u16,
    #[serde(default)]
    home_runs: u16,
    #[serde(default)]
    earned_runs: u16,
    #[serde(default)]
    hits: u16,
    #[serde(default)]
    batters_faced: u16,
    #[serde(default)]
    number_of_pitches: Option<u16>,
    #[serde(default)]
    ground_outs: u16,
    #[serde(default)]
    air_outs: u16,
}

fn parse_innings(value: &str) -> Result<f64> {
    let (whole, partial) = value.split_once('.').unwrap_or((value, "0"));
    let outs = match partial {
        "0" => 0.0,
        "1" => 1.0 / 3.0,
        "2" => 2.0 / 3.0,
        _ => {
            return Err(PnsError::Normalize(format!(
                "invalid innings pitched value {value}"
            )));
        }
    };
    Ok(whole
        .parse::<f64>()
        .map_err(|_| PnsError::Normalize(format!("invalid innings pitched value {value}")))?
        + outs)
}

pub fn normalize_game_log(json: &str) -> Result<Vec<PitcherStartLog>> {
    let response: StatsResponse = serde_json::from_str(json)?;
    let mut rows = Vec::new();
    for split in response.stats.into_iter().flat_map(|group| group.splits) {
        if split.stat.games_started != 1 {
            continue;
        }
        let balls_in_play_outs = split.stat.ground_outs + split.stat.air_outs;
        rows.push(PitcherStartLog {
            pitcher_id: split.player.id,
            pitcher_name: split.player.full_name,
            game_pk: split.game.game_pk,
            game_date: NaiveDate::parse_from_str(&split.date, "%Y-%m-%d")
                .map_err(|e| PnsError::Normalize(e.to_string()))?,
            team_id: split.team.id,
            opponent_id: split.opponent.id,
            opponent_name: split.opponent.name,
            home: split.is_home,
            innings_pitched: parse_innings(&split.stat.innings_pitched)?,
            strikeouts: split.stat.strike_outs,
            walks: split.stat.base_on_balls,
            home_runs: split.stat.home_runs,
            earned_runs: split.stat.earned_runs,
            hits: split.stat.hits,
            batters_faced: split.stat.batters_faced,
            pitch_count: split.stat.number_of_pitches,
            ground_ball_pct: (balls_in_play_outs > 0)
                .then(|| split.stat.ground_outs as f64 / balls_in_play_outs as f64),
            avg_fb_velo: None,
            historical_park_factor: None,
            historical_opponent_offense: None,
            pitch_hand: None,
        });
    }
    rows.sort_by_key(|row| (row.pitcher_id, row.game_date));
    Ok(rows)
}

pub fn normalize_fixture(json: &str) -> Result<Vec<PitcherStartLog>> {
    let mut rows: Vec<PitcherStartLog> = serde_json::from_str(json)?;
    rows.sort_by_key(|row| (row.pitcher_id, row.game_date));
    Ok(rows)
}

pub fn fixture_starts() -> Result<Vec<PitcherStartLog>> {
    normalize_fixture(NORMALIZED_FIXTURE)
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_mlb_log_and_excludes_relief_appearances() {
        let rows = super::normalize_game_log(include_str!(
            "../../../../data/fixtures/mlb_pitcher_game_log.json"
        ))
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pitcher_name, "Test Pitcher");
        assert!((rows[0].innings_pitched - 5.0 - 2.0 / 3.0).abs() < 0.001);
        assert_eq!(rows[1].batters_faced, 25);
    }

    #[test]
    fn normalized_fixture_is_stable() {
        let rows = super::fixture_starts().unwrap();
        assert_eq!(rows.len(), 16);
        assert!(rows.iter().all(|row| row.innings_pitched > 0.0));
    }
}
