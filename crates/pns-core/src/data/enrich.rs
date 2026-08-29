use super::types::PitcherStartLog;
use crate::{PnsError, Result};
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Default, Clone, Copy)]
struct RateStats {
    earned_runs: f64,
    innings: f64,
}
impl RateStats {
    fn add(&mut self, row: &PitcherStartLog) {
        self.earned_runs += row.earned_runs as f64;
        self.innings += row.innings_pitched;
    }
    fn rate(self, fallback: f64, prior_innings: f64) -> f64 {
        (self.earned_runs + fallback * prior_innings) / (self.innings + prior_innings)
    }
}

/// Adds context to each row using only games on earlier dates. Opponent strength
/// is starter ER/IP by opponent and pitcher hand; park environment is starter
/// ER/IP by home-team park proxy. Both are shrunk toward the prior league rate.
pub fn enrich_historical_context(rows: &mut [PitcherStartLog], hands: &HashMap<u32, char>) {
    rows.sort_by_key(|row| row.game_date);
    let mut league = RateStats::default();
    let mut opponents: HashMap<(u32, char), RateStats> = HashMap::new();
    let mut parks: HashMap<u32, RateStats> = HashMap::new();
    let mut index = 0;
    while index < rows.len() {
        let date = rows[index].game_date;
        let end = rows[index..]
            .iter()
            .position(|row| row.game_date > date)
            .map(|offset| index + offset)
            .unwrap_or(rows.len());
        let league_rate = if league.innings > 0.0 {
            league.earned_runs / league.innings
        } else {
            0.48
        };
        for row in &mut rows[index..end] {
            let hand = hands.get(&row.pitcher_id).copied().unwrap_or('R');
            row.pitch_hand = Some(hand);
            let home_team_id = if row.home {
                row.team_id
            } else {
                row.opponent_id
            };
            let opponent_rate = opponents
                .get(&(row.opponent_id, hand))
                .copied()
                .unwrap_or_default()
                .rate(league_rate, 60.0);
            let park_rate = parks
                .get(&home_team_id)
                .copied()
                .unwrap_or_default()
                .rate(league_rate, 120.0);
            row.historical_opponent_offense = Some((opponent_rate / league_rate).clamp(0.75, 1.30));
            row.historical_park_factor = Some((park_rate / league_rate).clamp(0.85, 1.15));
        }
        for row in &rows[index..end] {
            let hand = hands.get(&row.pitcher_id).copied().unwrap_or('R');
            let home_team_id = if row.home {
                row.team_id
            } else {
                row.opponent_id
            };
            league.add(row);
            opponents
                .entry((row.opponent_id, hand))
                .or_default()
                .add(row);
            parks.entry(home_team_id).or_default().add(row);
        }
        index = end;
    }
    rows.sort_by_key(|row| (row.pitcher_id, row.game_date));
}

#[derive(Deserialize)]
struct SavantPitch {
    pitch_type: String,
    game_date: String,
    release_speed: String,
}

pub fn normalize_savant_velocity(csv_text: &str) -> Result<HashMap<NaiveDate, f64>> {
    let cleaned = csv_text.trim_start_matches('\u{feff}');
    let mut reader = csv::Reader::from_reader(cleaned.as_bytes());
    let mut grouped: HashMap<NaiveDate, (f64, usize)> = HashMap::new();
    for record in reader.deserialize::<SavantPitch>() {
        let pitch = record?;
        if !matches!(pitch.pitch_type.as_str(), "FF" | "SI") {
            continue;
        }
        let date = NaiveDate::parse_from_str(&pitch.game_date, "%Y-%m-%d")
            .map_err(|error| PnsError::Normalize(error.to_string()))?;
        let velocity = match pitch.release_speed.parse::<f64>() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let entry = grouped.entry(date).or_default();
        entry.0 += velocity;
        entry.1 += 1;
    }
    Ok(grouped
        .into_iter()
        .filter(|(_, (_, count))| *count > 0)
        .map(|(date, (sum, count))| (date, sum / count as f64))
        .collect())
}

pub fn join_velocity(
    rows: &mut [PitcherStartLog],
    pitcher_id: u32,
    velocities: &HashMap<NaiveDate, f64>,
) {
    for row in rows.iter_mut().filter(|row| row.pitcher_id == pitcher_id) {
        row.avg_fb_velo = velocities.get(&row.game_date).copied();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn aggregates_fastballs_by_start_date() {
        let csv = "pitch_type,game_date,release_speed\nFF,2025-04-01,95.0\nSI,2025-04-01,93.0\nKC,2025-04-01,80.0\nFF,2025-04-07,96.0\n";
        let rows = super::normalize_savant_velocity(csv).unwrap();
        assert_eq!(
            rows[&chrono::NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()],
            94.0
        );
    }

    #[test]
    fn historical_context_never_uses_same_date() {
        let mut rows = crate::data::pitcher_logs::fixture_starts().unwrap();
        super::enrich_historical_context(&mut rows, &std::collections::HashMap::new());
        assert!(rows.iter().all(|row| row.historical_park_factor.is_some()));
        let first_date = rows.iter().map(|row| row.game_date).min().unwrap();
        assert!(
            rows.iter()
                .filter(|row| row.game_date == first_date)
                .all(|row| (row.historical_park_factor.unwrap() - 1.0).abs() < 1e-9)
        );
    }
}
