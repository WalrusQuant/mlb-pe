use crate::{DecisionCard, ModelInfo, PnsError, Result};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedPrediction {
    #[serde(default)]
    pub model_artifact_id: String,
    #[serde(default)]
    pub model_trained_through: String,
    pub pitcher_id: u32,
    pub pitcher_name: String,
    pub target_game_date: String,
    pub generated_at: String,
    pub projected_fip: f64,
    pub expected_runs: f64,
    pub expected_innings: f64,
    pub expected_strikeouts: f64,
    pub actual_fip: Option<f64>,
    pub actual_runs: Option<f64>,
    pub actual_innings: Option<f64>,
    pub actual_strikeouts: Option<f64>,
}

pub fn history_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("outputs/history/predictions.csv")
}

pub fn read_history(root: impl AsRef<Path>) -> Result<Vec<TrackedPrediction>> {
    let path = history_path(root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    csv::Reader::from_path(path)?
        .deserialize()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(PnsError::from)
}

pub fn record_predictions(
    cards: &[DecisionCard],
    model: &ModelInfo,
    root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let path = history_path(&root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut rows: BTreeMap<_, _> = read_history(&root)?
        .into_iter()
        .map(|row| ((row.target_game_date.clone(), row.pitcher_id), row))
        .collect();
    let generated_at = Utc::now().to_rfc3339();
    for card in cards {
        let key = (card.target_game_date.clone(), card.pitcher_id);
        let actual = rows.get(&key).cloned();
        rows.insert(
            key,
            TrackedPrediction {
                model_artifact_id: model.artifact_id.clone(),
                model_trained_through: model.trained_through.to_string(),
                pitcher_id: card.pitcher_id,
                pitcher_name: card.pitcher_name.clone(),
                target_game_date: card.target_game_date.clone(),
                generated_at: generated_at.clone(),
                projected_fip: card.projected_fip,
                expected_runs: card.expected_runs_base,
                expected_innings: card.expected_innings,
                expected_strikeouts: card.expected_strikeouts,
                actual_fip: actual.as_ref().and_then(|row| row.actual_fip),
                actual_runs: actual.as_ref().and_then(|row| row.actual_runs),
                actual_innings: actual.as_ref().and_then(|row| row.actual_innings),
                actual_strikeouts: actual.and_then(|row| row.actual_strikeouts),
            },
        );
    }
    write_history(&path, rows.into_values())?;
    Ok(path)
}

pub fn reconcile_history(root: impl AsRef<Path>) -> Result<Vec<TrackedPrediction>> {
    let path = history_path(&root);
    let mut rows = read_history(&root)?;
    let today = Utc::now().date_naive();
    let client = crate::data::MlbStatsClient::new()?;
    for row in &mut rows {
        if row.actual_innings.is_some() {
            continue;
        }
        let date = NaiveDate::parse_from_str(&row.target_game_date, "%Y-%m-%d")
            .map_err(|error| PnsError::Normalize(error.to_string()))?;
        if date >= today {
            continue;
        }
        if let Ok(starts) = client.pitcher_starts_range(row.pitcher_id, date, date)
            && let Some(start) = starts.into_iter().find(|start| start.game_date == date)
        {
            row.actual_fip = Some(start.fip().clamp(0.0, 10.0));
            row.actual_runs = Some(start.earned_runs as f64);
            row.actual_innings = Some(start.innings_pitched);
            row.actual_strikeouts = Some(start.strikeouts as f64);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_history(&path, rows.iter().cloned())?;
    Ok(rows)
}

fn write_history(path: &Path, rows: impl IntoIterator<Item = TrackedPrediction>) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn records_predictions_without_duplicate_keys() {
        let root = std::env::temp_dir().join(format!("pns-tracking-{}", std::process::id()));
        let cards = crate::score_date_fixture("2026-07-17").unwrap();
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let model = crate::model::artifact::ModelArtifact::train(&starts)
            .unwrap()
            .info();
        super::record_predictions(&cards, &model, &root).unwrap();
        super::record_predictions(&cards, &model, &root).unwrap();
        let rows = super::read_history(&root).unwrap();
        assert_eq!(rows.len(), cards.len());
        assert!(
            rows.iter()
                .all(|row| row.model_artifact_id == model.artifact_id)
        );
        std::fs::remove_dir_all(root).ok();
    }
}
