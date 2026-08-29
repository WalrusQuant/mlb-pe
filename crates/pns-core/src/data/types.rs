use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitcherStartLog {
    pub pitcher_id: u32,
    pub pitcher_name: String,
    pub game_pk: u32,
    pub game_date: NaiveDate,
    pub team_id: u32,
    pub opponent_id: u32,
    #[serde(default)]
    pub opponent_name: Option<String>,
    pub home: bool,
    pub innings_pitched: f64,
    pub strikeouts: u16,
    pub walks: u16,
    pub home_runs: u16,
    pub earned_runs: u16,
    pub hits: u16,
    pub batters_faced: u16,
    #[serde(default)]
    pub pitch_count: Option<u16>,
    #[serde(default)]
    pub ground_ball_pct: Option<f64>,
    #[serde(default)]
    pub avg_fb_velo: Option<f64>,
    #[serde(default)]
    pub historical_park_factor: Option<f64>,
    #[serde(default)]
    pub historical_opponent_offense: Option<f64>,
    #[serde(default)]
    pub pitch_hand: Option<char>,
}

impl PitcherStartLog {
    pub fn fip(&self) -> f64 {
        ((13.0 * self.home_runs as f64 + 3.0 * self.walks as f64 - 2.0 * self.strikeouts as f64)
            / self.innings_pitched.max(0.1))
            + 3.20
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStartContext {
    pub pitcher_id: u32,
    pub pitcher_name: String,
    pub game_date: NaiveDate,
    pub opponent_id: u32,
    pub home_team_id: u32,
    pub home: bool,
    pub park_factor: f64,
    pub opponent_offense: f64,
    pub pitcher_team_name: String,
    pub opponent_team_name: String,
    pub venue_name: String,
    pub game_time: Option<String>,
    pub pitcher_hand: Option<char>,
}
