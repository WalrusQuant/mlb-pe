use crate::data::types::{NextStartContext, PitcherStartLog};
use crate::{PnsError, Result};
use chrono::NaiveDate;
use std::collections::BTreeMap;

pub const FEATURE_SCHEMA_VERSION: u32 = 1;
pub const FEATURE_NAMES: [&str; 16] = [
    "intercept",
    "recent_k_rate",
    "recent_bb_rate",
    "recent_hr_per_9",
    "recent_whip",
    "recent_er_per_9",
    "recent_ground_ball_pct",
    "recent_fastball_velocity",
    "home",
    "park_factor",
    "opponent_offense_factor",
    "rest_days",
    "starts_used",
    "recent_innings_per_start",
    "recent_strikeouts_per_start",
    "recent_pitch_count",
];

#[derive(Debug, Clone)]
pub struct FeatureRow {
    pub pitcher_id: u32,
    pub as_of: NaiveDate,
    pub values: Vec<f64>,
    pub starts_used: usize,
    pub target_fip: Option<f64>,
    pub target_runs: Option<f64>,
    pub target_strikeouts: Option<f64>,
    pub target_innings: Option<f64>,
}

fn build(
    prior: &[&PitcherStartLog],
    pitcher_id: u32,
    as_of: NaiveDate,
    home: bool,
    park: f64,
    offense: f64,
) -> Result<FeatureRow> {
    if prior.is_empty() {
        return Err(PnsError::Feature(format!(
            "no prior starts for pitcher {pitcher_id}"
        )));
    }
    let n = prior.len() as f64;
    let bf = prior
        .iter()
        .map(|s| s.batters_faced as f64)
        .sum::<f64>()
        .max(1.0);
    let ip = prior
        .iter()
        .map(|s| s.innings_pitched)
        .sum::<f64>()
        .max(0.1);
    let k_rate = prior.iter().map(|s| s.strikeouts as f64).sum::<f64>() / bf;
    let bb_rate = prior.iter().map(|s| s.walks as f64).sum::<f64>() / bf;
    let hr9 = prior.iter().map(|s| s.home_runs as f64).sum::<f64>() * 9.0 / ip;
    let whip = (prior.iter().map(|s| (s.hits + s.walks) as f64).sum::<f64>()) / ip;
    let er9 = prior.iter().map(|s| s.earned_runs as f64).sum::<f64>() * 9.0 / ip;
    let average_innings = ip / n;
    let average_strikeouts = prior.iter().map(|s| s.strikeouts as f64).sum::<f64>() / n;
    let pitch_counts: Vec<_> = prior.iter().filter_map(|s| s.pitch_count).collect();
    let average_pitch_count = if pitch_counts.is_empty() {
        85.0
    } else {
        pitch_counts.iter().map(|value| *value as f64).sum::<f64>() / pitch_counts.len() as f64
    };
    let ground_ball_pct = {
        let values: Vec<_> = prior
            .iter()
            .filter_map(|start| start.ground_ball_pct)
            .collect();
        if values.is_empty() {
            0.43
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    };
    let average_fastball_velocity = {
        let values: Vec<_> = prior.iter().filter_map(|start| start.avg_fb_velo).collect();
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    };
    let last_date = prior.iter().map(|s| s.game_date).max().unwrap();
    let rest_days = (as_of - last_date).num_days().clamp(3, 10) as f64;
    Ok(FeatureRow {
        pitcher_id,
        as_of,
        values: vec![
            1.0,
            k_rate,
            bb_rate,
            hr9,
            whip,
            er9,
            ground_ball_pct,
            average_fastball_velocity,
            home as u8 as f64,
            park,
            offense,
            rest_days,
            n,
            average_innings,
            average_strikeouts,
            average_pitch_count,
        ],
        starts_used: prior.len(),
        target_fip: None,
        target_runs: None,
        target_strikeouts: None,
        target_innings: None,
    })
}

pub fn historical_rows(starts: &[PitcherStartLog], window: usize) -> Result<Vec<FeatureRow>> {
    let mut groups: BTreeMap<u32, Vec<&PitcherStartLog>> = BTreeMap::new();
    for start in starts {
        groups.entry(start.pitcher_id).or_default().push(start);
    }
    let mut rows = Vec::new();
    for (id, group) in groups {
        for i in 3..group.len() {
            let current = group[i];
            let from = i.saturating_sub(window);
            let prior = &group[from..i];
            let mut row = build(
                prior,
                id,
                current.game_date,
                current.home,
                current.historical_park_factor.unwrap_or(1.0),
                current.historical_opponent_offense.unwrap_or(1.0),
            )?;
            row.target_fip = Some(current.fip().clamp(0.0, 10.0));
            row.target_runs = Some(current.earned_runs as f64);
            row.target_strikeouts = Some(current.strikeouts as f64);
            row.target_innings = Some(current.innings_pitched);
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn next_start_row(
    starts: &[PitcherStartLog],
    context: &NextStartContext,
    window: usize,
) -> Result<FeatureRow> {
    let eligible: Vec<_> = starts
        .iter()
        .filter(|s| s.pitcher_id == context.pitcher_id && s.game_date < context.game_date)
        .collect();
    let from = eligible.len().saturating_sub(window);
    build(
        &eligible[from..],
        context.pitcher_id,
        context.game_date,
        context.home,
        context.park_factor,
        context.opponent_offense,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn historical_features_never_include_current_start() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let rows = super::historical_rows(&starts, 5).unwrap();
        assert_eq!(rows.len(), 10);
        assert!(rows.iter().all(|r| r.starts_used <= 5));
        let first = &rows[0];
        assert_eq!(first.starts_used, 3);
    }
}
