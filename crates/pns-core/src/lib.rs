pub mod card;
pub mod data;
pub mod error;
pub mod features;
pub mod model;
pub mod tracking;

use chrono::Datelike;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use card::{Confidence, DecisionCard, Projection};
pub use error::{PnsError, Result};
pub use model::artifact::ModelInfo;

#[derive(Debug, serde::Serialize)]
pub struct ScoredSlate {
    pub cards: Vec<DecisionCard>,
    pub validation: model::ValidationMetrics,
    pub model: ModelInfo,
    pub generated_at: String,
}

#[derive(serde::Serialize)]
struct PredictionCsv<'a> {
    model_artifact_id: Option<&'a str>,
    model_trained_through: Option<String>,
    pitcher_id: u32,
    pitcher_name: &'a str,
    pitcher_team: &'a str,
    opponent_team: &'a str,
    target_game_date: &'a str,
    projected_fip: f64,
    expected_runs_low: f64,
    expected_runs_p25: f64,
    expected_runs_base: f64,
    expected_runs_p75: f64,
    expected_runs_high: f64,
    expected_strikeouts: f64,
    expected_innings: f64,
    expected_strikeouts_low: f64,
    expected_strikeouts_high: f64,
    expected_innings_low: f64,
    expected_innings_high: f64,
    confidence: Confidence,
    recent_k_rate: f64,
    recent_bb_rate: f64,
    recent_hr_per_9: f64,
    recent_whip: f64,
    recent_er_per_9: f64,
    recent_ground_ball_pct: f64,
    recent_fastball_velocity: Option<f64>,
    park_factor: f64,
    opponent_offense_factor: f64,
    judgment: &'a str,
    team_model_input: Option<f64>,
}

impl<'a> PredictionCsv<'a> {
    fn new(card: &'a DecisionCard, model: Option<&'a ModelInfo>) -> Self {
        Self {
            model_artifact_id: model.map(|info| info.artifact_id.as_str()),
            model_trained_through: model.map(|info| info.trained_through.to_string()),
            pitcher_id: card.pitcher_id,
            pitcher_name: &card.pitcher_name,
            pitcher_team: &card.pitcher_team_name,
            opponent_team: &card.opponent_team_name,
            target_game_date: &card.target_game_date,
            projected_fip: card.projected_fip,
            expected_runs_low: card.expected_runs_low,
            expected_runs_p25: card.expected_runs_p25,
            expected_runs_base: card.expected_runs_base,
            expected_runs_p75: card.expected_runs_p75,
            expected_runs_high: card.expected_runs_high,
            expected_strikeouts: card.expected_strikeouts,
            expected_innings: card.expected_innings,
            expected_strikeouts_low: card.expected_strikeouts_low,
            expected_strikeouts_high: card.expected_strikeouts_high,
            expected_innings_low: card.expected_innings_low,
            expected_innings_high: card.expected_innings_high,
            confidence: card.confidence,
            recent_k_rate: card.recent_k_rate,
            recent_bb_rate: card.recent_bb_rate,
            recent_hr_per_9: card.recent_hr_per_9,
            recent_whip: card.recent_whip,
            recent_er_per_9: card.recent_er_per_9,
            recent_ground_ball_pct: card.recent_ground_ball_pct,
            recent_fastball_velocity: card.recent_fastball_velocity,
            park_factor: card.park_factor,
            opponent_offense_factor: card.opponent_offense_factor,
            judgment: &card.judgment,
            team_model_input: card.team_model_input,
        }
    }
}

/// Fetch and score the probable starters MLB lists for the requested date.
/// Network responses are normalized before reaching feature/model code.
pub fn score_date(date: &str) -> Result<Vec<DecisionCard>> {
    Ok(score_date_detailed(date)?.cards)
}

pub fn score_date_detailed(date: &str) -> Result<ScoredSlate> {
    let target_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| PnsError::Feature(format!("invalid date {date}: {e}")))?;
    let artifact = load_model_artifact()?;
    if target_date <= artifact.training.data_through {
        return Err(PnsError::Model(format!(
            "model artifact {} was trained through {}; choose a later slate or train a leakage-safe historical artifact",
            artifact.artifact_id, artifact.training.data_through
        )));
    }
    let client = data::MlbStatsClient::new()?;
    let mut contexts = client.probable_starters(target_date)?;
    let pitcher_ids: Vec<_> = contexts.iter().map(|context| context.pitcher_id).collect();
    let hands = client.pitcher_hands(&pitcher_ids)?;
    let context_season = if target_date.month() <= 4 {
        target_date.year() - 1
    } else {
        target_date.year()
    };
    let slate_context = client.slate_context(context_season)?;
    for context in &mut contexts {
        context.pitcher_hand = hands.get(&context.pitcher_id).copied();
        context.park_factor = slate_context.park_factor(context.home_team_id);
        context.opponent_offense = slate_context.opponent_factor(
            context.opponent_id,
            hands.get(&context.pitcher_id).copied().unwrap_or('R'),
        );
    }
    let histories: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = contexts
            .iter()
            .map(|context| {
                let client = client.clone();
                scope.spawn(move || {
                    let mut rows = client.pitcher_starts(context.pitcher_id, target_date)?;
                    if let Ok(velocities) = client.savant_fastball_velocity(
                        context.pitcher_id,
                        target_date - chrono::Duration::days(400),
                        target_date - chrono::Duration::days(1),
                    ) {
                        data::enrich::join_velocity(&mut rows, context.pitcher_id, &velocities);
                    }
                    Ok::<_, PnsError>(rows)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("pitcher history worker panicked"))
            .collect()
    });
    let mut starts = Vec::new();
    let mut failures = Vec::new();
    for (context, history) in contexts.iter().zip(histories) {
        match history {
            Ok(mut rows) => starts.append(&mut rows),
            Err(error) => failures.push(format!("{}: {error}", context.pitcher_name)),
        }
    }
    if starts.is_empty() {
        return Err(PnsError::Data(format!(
            "no usable pitcher histories were returned: {}",
            failures.join("; ")
        )));
    }
    let deduplicated: BTreeMap<_, _> = starts
        .into_iter()
        .map(|row| ((row.pitcher_id, row.game_pk), row))
        .collect();
    starts = deduplicated.into_values().collect();
    for row in &mut starts {
        if let Some(hand) = hands.get(&row.pitcher_id) {
            row.pitch_hand = Some(*hand);
        }
    }
    let all_hands: std::collections::HashMap<_, _> = starts
        .iter()
        .filter_map(|row| row.pitch_hand.map(|hand| (row.pitcher_id, hand)))
        .collect();
    data::enrich::enrich_historical_context(&mut starts, &all_hands);
    score_normalized_detailed(&starts, &contexts, &artifact)
}

fn find_model_artifact() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PNS_MODEL_ARTIFACT") {
        let path = PathBuf::from(path);
        return Some(path);
    }
    let current = std::env::current_dir().ok()?;
    [
        current.join("models/pns-model-v1.json"),
        current.join("../models/pns-model-v1.json"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn load_model_artifact() -> Result<model::artifact::ModelArtifact> {
    if let Some(path) = find_model_artifact() {
        return model::artifact::ModelArtifact::load(path);
    }
    model::artifact::ModelArtifact::from_json(include_str!("../../../models/pns-model-v1.json"))
}

#[cfg(test)]
fn score_normalized(
    starts: &[data::types::PitcherStartLog],
    contexts: &[data::types::NextStartContext],
    artifact: &model::artifact::ModelArtifact,
) -> Result<Vec<DecisionCard>> {
    Ok(score_normalized_detailed(starts, contexts, artifact)?.cards)
}

fn score_normalized_detailed(
    starts: &[data::types::PitcherStartLog],
    contexts: &[data::types::NextStartContext],
    artifact: &model::artifact::ModelArtifact,
) -> Result<ScoredSlate> {
    artifact.validate()?;
    let cards: Vec<_> = contexts
        .iter()
        .filter_map(|context| {
            let row = match features::next_start_row(starts, context, 5) {
                Ok(row) => row,
                Err(_) => return None,
            };
            Some(artifact.score(&row).map(|projection| {
                card::build_card(
                    context,
                    projection,
                    &row,
                    starts,
                    artifact.validation.confidence_calibration,
                )
            }))
        })
        .collect::<Result<_>>()?;
    if cards.is_empty() {
        return Err(PnsError::Data(
            "probable starters were listed, but none had enough prior MLB starts to score".into(),
        ));
    }
    Ok(ScoredSlate {
        cards,
        validation: artifact.validation,
        model: artifact.info(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn score_date_fixture(date: &str) -> Result<Vec<DecisionCard>> {
    Ok(score_date_fixture_detailed(date)?.cards)
}

pub fn score_date_fixture_detailed(date: &str) -> Result<ScoredSlate> {
    let target_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| PnsError::Feature(format!("invalid date {date}: {e}")))?;
    let starts = data::pitcher_logs::fixture_starts()?;
    let contexts = vec![
        data::types::NextStartContext {
            pitcher_id: 660271,
            pitcher_name: "Fixture Ace".into(),
            game_date: target_date,
            opponent_id: 147,
            home_team_id: 120,
            home: true,
            park_factor: 1.0,
            opponent_offense: 1.0,
            pitcher_team_name: "Fixture Club".into(),
            opponent_team_name: "Fixture Opponent".into(),
            venue_name: "Fixture Park".into(),
            game_time: None,
            pitcher_hand: Some('R'),
        },
        data::types::NextStartContext {
            pitcher_id: 621141,
            pitcher_name: "Fixture Southpaw".into(),
            game_date: target_date,
            opponent_id: 119,
            home_team_id: 119,
            home: false,
            park_factor: 1.0,
            opponent_offense: 1.0,
            pitcher_team_name: "Fixture Club".into(),
            opponent_team_name: "Fixture Opponent".into(),
            venue_name: "Fixture Park".into(),
            game_time: None,
            pitcher_hand: Some('L'),
        },
    ];
    let artifact = load_model_artifact()?;
    score_normalized_detailed(&starts, &contexts, &artifact)
}

pub fn export_predictions(cards: &[DecisionCard], root: impl AsRef<Path>) -> Result<PathBuf> {
    export_predictions_with_model(cards, None, root)
}

pub fn export_predictions_with_model(
    cards: &[DecisionCard],
    model: Option<&ModelInfo>,
    root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let output_dir = root.as_ref().join("outputs");
    let cards_dir = output_dir.join("cards");
    std::fs::create_dir_all(&cards_dir)?;
    for entry in std::fs::read_dir(&cards_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            std::fs::remove_file(path)?;
        }
    }
    let csv_path = output_dir.join("predictions.csv");
    let mut writer = csv::Writer::from_path(&csv_path)?;
    for card in cards {
        writer.serialize(PredictionCsv::new(card, model))?;
        let slug = card.pitcher_name.to_lowercase().replace([' ', '.'], "-");
        std::fs::write(
            cards_dir.join(format!("{slug}.md")),
            card::render_markdown(card),
        )?;
    }
    writer.flush()?;
    Ok(csv_path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn scores_and_exports_two_fixture_pitchers() {
        let cards = super::score_date_fixture("2026-07-17").unwrap();
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().all(|card| card.starts_used == 5));
        assert!(cards.iter().all(|card| card.recent_k_rate > 0.0));
        assert!(cards.iter().all(|card| card.park_factor > 0.0));
        assert!(cards.iter().all(|card| !card.drivers.is_empty()));
        assert!(cards.iter().all(|card| card.season_form.starts >= 5));
        assert!(cards.iter().all(|card| card.recent_starts.len() == 5));
        assert!(cards.iter().all(|card| !card.outlook_summary.is_empty()));
        assert!(cards.iter().all(|card| card.starter_status == "Probable"));
        assert!(cards.iter().all(|card| card.data_through != "Unavailable"));
        assert!(cards.iter().all(|card| card.history_starts >= 5));
        assert!(cards.iter().all(|card| card.expected_innings > 0.0));
        assert!(cards.iter().all(|card| card.expected_strikeouts >= 0.0));
        assert!(cards.iter().all(|card| {
            card.recent_starts
                .iter()
                .all(|start| start.batters_faced > 0)
        }));
        assert!(
            cards
                .iter()
                .all(|card| card.expected_runs_low <= card.expected_runs_p25
                    && card.expected_runs_p25 <= card.expected_runs_base
                    && card.expected_runs_base <= card.expected_runs_p75
                    && card.expected_runs_p75 <= card.expected_runs_high)
        );
        assert!(
            cards
                .iter()
                .all(|c| c.expected_runs_low < c.expected_runs_high)
        );
        let root = std::env::temp_dir().join("pns-core-export-test");
        let path = super::export_predictions(&cards, &root).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn frozen_artifact_scores_are_independent_of_unrelated_history() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let artifact = crate::model::artifact::ModelArtifact::train(&starts).unwrap();
        let target_date = chrono::NaiveDate::from_ymd_opt(2026, 7, 16).unwrap();
        let context = crate::data::types::NextStartContext {
            pitcher_id: 660271,
            pitcher_name: "Fixture Ace".into(),
            game_date: target_date,
            opponent_id: 147,
            home_team_id: 120,
            home: true,
            park_factor: 1.0,
            opponent_offense: 1.0,
            pitcher_team_name: "Fixture Club".into(),
            opponent_team_name: "Fixture Opponent".into(),
            venue_name: "Fixture Park".into(),
            game_time: None,
            pitcher_hand: Some('R'),
        };
        let first = super::score_normalized(&starts, std::slice::from_ref(&context), &artifact)
            .unwrap()
            .remove(0);
        let mut expanded = starts.clone();
        let mut unrelated = starts[0].clone();
        unrelated.pitcher_id = 999_999;
        unrelated.game_pk = 999_999;
        expanded.push(unrelated);
        let second = super::score_normalized(&expanded, &[context], &artifact)
            .unwrap()
            .remove(0);
        assert_eq!(first.projected_fip, second.projected_fip);
        assert_eq!(first.expected_runs_base, second.expected_runs_base);
        assert_eq!(first.confidence, second.confidence);
    }
}
