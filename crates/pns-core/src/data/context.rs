use crate::{PnsError, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SlateContext {
    vs_left_ops: HashMap<u32, f64>,
    vs_right_ops: HashMap<u32, f64>,
    home_runs_per_game: HashMap<u32, f64>,
    away_runs_per_game: HashMap<u32, f64>,
    league_vs_left_ops: f64,
    league_vs_right_ops: f64,
}

impl SlateContext {
    pub fn opponent_factor(&self, team_id: u32, pitcher_hand: char) -> f64 {
        let (values, average) = if pitcher_hand == 'L' {
            (&self.vs_left_ops, self.league_vs_left_ops)
        } else {
            (&self.vs_right_ops, self.league_vs_right_ops)
        };
        values
            .get(&team_id)
            .map(|ops| ops / average)
            .unwrap_or(1.0)
            .clamp(0.75, 1.30)
    }

    pub fn park_factor(&self, home_team_id: u32) -> f64 {
        match (
            self.home_runs_per_game.get(&home_team_id),
            self.away_runs_per_game.get(&home_team_id),
        ) {
            (Some(home), Some(away)) if *away > 0.0 => (home / away).clamp(0.85, 1.15),
            _ => 1.0,
        }
    }
}

#[derive(Deserialize)]
struct PeopleResponse {
    people: Vec<Person>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Person {
    id: u32,
    pitch_hand: Hand,
}
#[derive(Deserialize)]
struct Hand {
    code: String,
}

pub fn normalize_pitcher_hands(json: &str) -> Result<HashMap<u32, char>> {
    let response: PeopleResponse = serde_json::from_str(json)?;
    Ok(response
        .people
        .into_iter()
        .filter_map(|person| {
            person
                .pitch_hand
                .code
                .chars()
                .next()
                .map(|hand| (person.id, hand))
        })
        .collect())
}

#[derive(Deserialize)]
struct TeamStatsResponse {
    #[serde(default)]
    stats: Vec<TeamStatsGroup>,
}
#[derive(Deserialize)]
struct TeamStatsGroup {
    #[serde(default)]
    splits: Vec<TeamSplit>,
}
#[derive(Deserialize)]
struct TeamSplit {
    team: Team,
    stat: TeamStat,
}
#[derive(Deserialize)]
struct Team {
    id: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamStat {
    ops: String,
    #[serde(default)]
    runs: u32,
    #[serde(default)]
    games_played: u32,
}

fn normalize_ops(json: &str) -> Result<HashMap<u32, f64>> {
    let response: TeamStatsResponse = serde_json::from_str(json)?;
    response
        .stats
        .into_iter()
        .flat_map(|group| group.splits)
        .map(|split| {
            let ops =
                split.stat.ops.parse::<f64>().map_err(|_| {
                    PnsError::Normalize(format!("invalid team OPS {}", split.stat.ops))
                })?;
            Ok((split.team.id, ops))
        })
        .collect()
}

fn normalize_runs_per_game(json: &str) -> Result<HashMap<u32, f64>> {
    let response: TeamStatsResponse = serde_json::from_str(json)?;
    Ok(response
        .stats
        .into_iter()
        .flat_map(|group| group.splits)
        .filter(|split| split.stat.games_played > 0)
        .map(|split| {
            (
                split.team.id,
                split.stat.runs as f64 / split.stat.games_played as f64,
            )
        })
        .collect())
}

fn mean(values: &HashMap<u32, f64>) -> Result<f64> {
    if values.is_empty() {
        return Err(PnsError::Normalize("team split response was empty".into()));
    }
    Ok(values.values().sum::<f64>() / values.len() as f64)
}

pub fn normalize_slate_context(
    vs_left: &str,
    vs_right: &str,
    home: &str,
    away: &str,
) -> Result<SlateContext> {
    let vs_left_ops = normalize_ops(vs_left)?;
    let vs_right_ops = normalize_ops(vs_right)?;
    Ok(SlateContext {
        league_vs_left_ops: mean(&vs_left_ops)?,
        league_vs_right_ops: mean(&vs_right_ops)?,
        vs_left_ops,
        vs_right_ops,
        home_runs_per_game: normalize_runs_per_game(home)?,
        away_runs_per_game: normalize_runs_per_game(away)?,
    })
}

#[cfg(test)]
mod tests {
    const LEFT: &str = r#"{"stats":[{"splits":[{"team":{"id":1},"stat":{"ops":".800","runs":80,"gamesPlayed":20}},{"team":{"id":2},"stat":{"ops":".600","runs":60,"gamesPlayed":20}}]}]}"#;
    const RIGHT: &str = r#"{"stats":[{"splits":[{"team":{"id":1},"stat":{"ops":".750","runs":80,"gamesPlayed":20}},{"team":{"id":2},"stat":{"ops":".750","runs":60,"gamesPlayed":20}}]}]}"#;
    const HOME: &str = r#"{"stats":[{"splits":[{"team":{"id":1},"stat":{"ops":".750","runs":90,"gamesPlayed":20}},{"team":{"id":2},"stat":{"ops":".700","runs":60,"gamesPlayed":20}}]}]}"#;
    const AWAY: &str = r#"{"stats":[{"splits":[{"team":{"id":1},"stat":{"ops":".700","runs":75,"gamesPlayed":20}},{"team":{"id":2},"stat":{"ops":".700","runs":60,"gamesPlayed":20}}]}]}"#;

    #[test]
    fn derives_opponent_and_park_factors() {
        let context = super::normalize_slate_context(LEFT, RIGHT, HOME, AWAY).unwrap();
        assert!((context.opponent_factor(1, 'L') - 0.8 / 0.7).abs() < 0.001);
        assert!((context.opponent_factor(1, 'R') - 1.0).abs() < 0.001);
        assert!((context.park_factor(1) - 1.15).abs() < 0.001);
    }

    #[test]
    fn normalizes_handedness() {
        let hands = super::normalize_pitcher_hands(
            r#"{"people":[{"id":10,"pitchHand":{"code":"L"}},{"id":11,"pitchHand":{"code":"R"}}]}"#,
        )
        .unwrap();
        assert_eq!(hands[&10], 'L');
        assert_eq!(hands[&11], 'R');
    }
}
