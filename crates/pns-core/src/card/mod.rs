use crate::data::types::{NextStartContext, PitcherStartLog};
use crate::features::FeatureRow;
use crate::model::ConfidenceCalibration;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Projection {
    pub projected_fip: f64,
    pub expected_runs_low: f64,
    pub expected_runs_p25: f64,
    pub expected_runs_base: f64,
    pub expected_runs_p75: f64,
    pub expected_runs_high: f64,
    pub posterior_width: f64,
    pub starts_used: usize,
    pub fip_posterior_sd: f64,
    pub runs_posterior_sd: f64,
    pub runs_epistemic_sd: f64,
    pub model_training_rows: usize,
    pub expected_strikeouts: f64,
    pub expected_innings: f64,
    pub expected_strikeouts_low: f64,
    pub expected_strikeouts_high: f64,
    pub expected_innings_low: f64,
    pub expected_innings_high: f64,
    pub strikeouts_posterior_sd: f64,
    pub innings_posterior_sd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDriver {
    pub label: String,
    pub impact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitchingLine {
    pub starts: usize,
    pub k_rate: f64,
    pub bb_rate: f64,
    pub hr_per_9: f64,
    pub whip: f64,
    pub er_per_9: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentStart {
    pub game_date: String,
    pub opponent_name: String,
    pub home: bool,
    pub innings_pitched: f64,
    pub earned_runs: u16,
    pub strikeouts: u16,
    pub walks: u16,
    pub pitch_count: Option<u16>,
    pub batters_faced: u16,
    pub hits: u16,
    pub home_runs: u16,
    pub fip: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionCard {
    pub pitcher_id: u32,
    pub pitcher_name: String,
    pub target_game_date: String,
    pub projected_fip: f64,
    pub expected_runs_low: f64,
    pub expected_runs_p25: f64,
    pub expected_runs_base: f64,
    pub expected_runs_p75: f64,
    pub expected_runs_high: f64,
    pub expected_strikeouts: f64,
    pub expected_innings: f64,
    pub expected_strikeouts_low: f64,
    pub expected_strikeouts_high: f64,
    pub expected_innings_low: f64,
    pub expected_innings_high: f64,
    pub confidence: Confidence,
    pub judgment: String,
    pub outlook_summary: String,
    pub prop_lean: Option<String>,
    pub team_model_input: Option<f64>,
    pub starts_used: usize,
    pub recent_k_rate: f64,
    pub recent_bb_rate: f64,
    pub recent_hr_per_9: f64,
    pub recent_whip: f64,
    pub recent_er_per_9: f64,
    pub recent_ground_ball_pct: f64,
    pub recent_fastball_velocity: Option<f64>,
    pub rest_days: u16,
    pub home: bool,
    pub park_factor: f64,
    pub opponent_offense_factor: f64,
    pub pitcher_team_name: String,
    pub opponent_team_name: String,
    pub venue_name: String,
    pub game_time: Option<String>,
    pub pitcher_hand: Option<char>,
    pub drivers: Vec<ModelDriver>,
    pub season_form: PitchingLine,
    pub recent_starts: Vec<RecentStart>,
    pub starter_status: String,
    pub data_through: String,
    pub history_starts: usize,
    pub data_warnings: Vec<String>,
}

pub fn build_card(
    context: &NextStartContext,
    p: Projection,
    row: &FeatureRow,
    starts: &[PitcherStartLog],
    calibration: ConfidenceCalibration,
) -> DecisionCard {
    let confidence = confidence_for(&p, calibration);
    let quality = if p.projected_fip < 3.7 {
        "Strong run-prevention outlook"
    } else if p.projected_fip < 4.5 {
        "League-average run-prevention outlook"
    } else {
        "Elevated run-risk outlook"
    };
    let judgment = format!(
        "{quality} with {} confidence; use the {:.1}–{:.1} run band in decisions.",
        confidence_label(confidence),
        p.expected_runs_low,
        p.expected_runs_high
    );
    let eligible: Vec<_> = starts
        .iter()
        .filter(|start| {
            start.pitcher_id == context.pitcher_id && start.game_date < context.game_date
        })
        .collect();
    let season_rows: Vec<_> = eligible
        .iter()
        .copied()
        .filter(|start| start.game_date.year() == context.game_date.year())
        .collect();
    let season_form = pitching_line(if season_rows.is_empty() {
        &eligible
    } else {
        &season_rows
    });
    let recent_starts = eligible
        .iter()
        .rev()
        .take(5)
        .map(|start| RecentStart {
            game_date: start.game_date.to_string(),
            opponent_name: start
                .opponent_name
                .clone()
                .unwrap_or_else(|| format!("Team {}", start.opponent_id)),
            home: start.home,
            innings_pitched: round2(start.innings_pitched),
            earned_runs: start.earned_runs,
            strikeouts: start.strikeouts,
            walks: start.walks,
            pitch_count: start.pitch_count,
            batters_faced: start.batters_faced,
            hits: start.hits,
            home_runs: start.home_runs,
            fip: round2(start.fip().clamp(0.0, 10.0)),
        })
        .collect();
    let direction = if p.projected_fip + 0.25 < season_form.er_per_9 {
        "better than his season run-prevention rate"
    } else if p.projected_fip > season_form.er_per_9 + 0.25 {
        "worse than his season run-prevention rate"
    } else {
        "close to his season run-prevention rate"
    };
    let context_read = if row.values[9] > 1.02 || row.values[10] > 1.02 {
        "The matchup environment adds downside."
    } else if row.values[9] < 0.98 || row.values[10] < 0.98 {
        "The matchup environment is favorable."
    } else {
        "The matchup environment is approximately neutral."
    };
    let outlook_summary = format!(
        "The model expects {:.1} runs, with a central outlook {direction}. {context_read}",
        p.expected_runs_base
    );
    let data_through = eligible
        .iter()
        .map(|start| start.game_date)
        .max()
        .map(|date| date.to_string())
        .unwrap_or_else(|| "Unavailable".into());
    let mut data_warnings = Vec::new();
    if row.values[7] <= 0.0 {
        data_warnings
            .push("Fastball velocity is unavailable; the model used its neutral fallback.".into());
    }
    if eligible.len() < 5 {
        data_warnings.push(format!(
            "Only {} prior MLB starts were available.",
            eligible.len()
        ));
    }
    if context.game_time.is_none() {
        data_warnings.push("Game time has not been posted by MLB.".into());
    }
    let mut drivers = Vec::new();
    push_driver(
        &mut drivers,
        row.values[1] >= 0.25,
        row.values[1] <= 0.18,
        "Elevated strikeout rate",
        "Low strikeout rate",
    );
    push_driver(
        &mut drivers,
        row.values[2] <= 0.07,
        row.values[2] >= 0.10,
        "Strong walk suppression",
        "Elevated walk rate",
    );
    if row.values[7] > 0.0 {
        push_driver(
            &mut drivers,
            row.values[7] >= 95.0,
            row.values[7] <= 91.0,
            "Above-average fastball velocity",
            "Below-average fastball velocity",
        );
    }
    push_driver(
        &mut drivers,
        row.values[9] < 0.98,
        row.values[9] > 1.02,
        "Pitcher-friendly park",
        "Hitter-friendly park",
    );
    push_driver(
        &mut drivers,
        row.values[10] < 0.98,
        row.values[10] > 1.02,
        "Favorable opponent split",
        "Difficult opponent split",
    );
    if drivers.is_empty() {
        drivers.push(ModelDriver {
            label: "Mostly neutral form and context".into(),
            impact: "neutral".into(),
        });
    }
    DecisionCard {
        pitcher_id: context.pitcher_id,
        pitcher_name: context.pitcher_name.clone(),
        target_game_date: context.game_date.to_string(),
        projected_fip: round2(p.projected_fip),
        expected_runs_low: round2(p.expected_runs_low),
        expected_runs_p25: round2(p.expected_runs_p25),
        expected_runs_base: round2(p.expected_runs_base),
        expected_runs_p75: round2(p.expected_runs_p75),
        expected_runs_high: round2(p.expected_runs_high),
        expected_strikeouts: round2(p.expected_strikeouts),
        expected_innings: round2(p.expected_innings),
        expected_strikeouts_low: round2(p.expected_strikeouts_low),
        expected_strikeouts_high: round2(p.expected_strikeouts_high),
        expected_innings_low: round2(p.expected_innings_low),
        expected_innings_high: round2(p.expected_innings_high),
        confidence,
        judgment,
        outlook_summary,
        prop_lean: None,
        team_model_input: Some(round2(p.expected_runs_base)),
        starts_used: row.starts_used,
        recent_k_rate: round3(row.values[1]),
        recent_bb_rate: round3(row.values[2]),
        recent_hr_per_9: round2(row.values[3]),
        recent_whip: round2(row.values[4]),
        recent_er_per_9: round2(row.values[5]),
        recent_ground_ball_pct: round3(row.values[6]),
        recent_fastball_velocity: (row.values[7] > 0.0).then(|| round2(row.values[7])),
        home: row.values[8] > 0.5,
        park_factor: round3(row.values[9]),
        opponent_offense_factor: round3(row.values[10]),
        rest_days: row.values[11].round().max(0.0) as u16,
        pitcher_team_name: context.pitcher_team_name.clone(),
        opponent_team_name: context.opponent_team_name.clone(),
        venue_name: context.venue_name.clone(),
        game_time: context.game_time.clone(),
        pitcher_hand: context.pitcher_hand,
        drivers,
        season_form,
        recent_starts,
        starter_status: "Probable".into(),
        data_through,
        history_starts: eligible.len(),
        data_warnings,
    }
}

fn confidence_for(p: &Projection, calibration: ConfidenceCalibration) -> Confidence {
    let score =
        calibration.epistemic_score(p.runs_epistemic_sd, p.model_training_rows, p.starts_used);
    confidence_for_score(score, p.starts_used, calibration)
}

fn confidence_for_score(
    score: f64,
    starts_used: usize,
    calibration: ConfidenceCalibration,
) -> Confidence {
    if !score.is_finite() || !calibration.has_valid_thresholds() {
        return Confidence::Low;
    }
    if !calibration.tiers_validated {
        return if starts_used >= 5 && score <= calibration.medium_max_normalized_epistemic_score {
            Confidence::Medium
        } else {
            Confidence::Low
        };
    }
    if starts_used == 5 && score <= calibration.high_max_normalized_epistemic_score {
        Confidence::High
    } else if starts_used == 5 && score <= calibration.medium_max_normalized_epistemic_score {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn pitching_line(rows: &[&PitcherStartLog]) -> PitchingLine {
    let bf = rows
        .iter()
        .map(|s| s.batters_faced as f64)
        .sum::<f64>()
        .max(1.0);
    let ip = rows.iter().map(|s| s.innings_pitched).sum::<f64>().max(0.1);
    PitchingLine {
        starts: rows.len(),
        k_rate: round3(rows.iter().map(|s| s.strikeouts as f64).sum::<f64>() / bf),
        bb_rate: round3(rows.iter().map(|s| s.walks as f64).sum::<f64>() / bf),
        hr_per_9: round2(rows.iter().map(|s| s.home_runs as f64).sum::<f64>() * 9.0 / ip),
        whip: round2(rows.iter().map(|s| (s.hits + s.walks) as f64).sum::<f64>() / ip),
        er_per_9: round2(rows.iter().map(|s| s.earned_runs as f64).sum::<f64>() * 9.0 / ip),
    }
}

fn push_driver(
    drivers: &mut Vec<ModelDriver>,
    positive: bool,
    negative: bool,
    positive_label: &str,
    negative_label: &str,
) {
    if positive {
        drivers.push(ModelDriver {
            label: positive_label.into(),
            impact: "positive".into(),
        });
    } else if negative {
        drivers.push(ModelDriver {
            label: negative_label.into(),
            impact: "negative".into(),
        });
    }
}

fn confidence_label(value: Confidence) -> &'static str {
    match value {
        Confidence::High => "high",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    }
}
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

pub fn render_markdown(card: &DecisionCard) -> String {
    format!(
        "# {} — {}\n\n- Projected FIP: **{:.2}**\n- Expected runs: **{:.2} / {:.2} / {:.2}**\n- Confidence: **{}**\n- Recent form ({} starts): **{:.1}% K, {:.1}% BB, {:.2} WHIP, {:.2} ER/9**\n- Shape: **{:.1}% GB, {} mph fastball**\n- Context: **{} days rest, park {:.3}, opponent {:.3}**\n- Team-model input: **{:.2}**\n\n{}\n",
        card.pitcher_name,
        card.target_game_date,
        card.projected_fip,
        card.expected_runs_low,
        card.expected_runs_base,
        card.expected_runs_high,
        confidence_label(card.confidence),
        card.starts_used,
        card.recent_k_rate * 100.0,
        card.recent_bb_rate * 100.0,
        card.recent_whip,
        card.recent_er_per_9,
        card.recent_ground_ball_pct * 100.0,
        card.recent_fastball_velocity
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "n/a".into()),
        card.rest_days,
        card.park_factor,
        card.opponent_offense_factor,
        card.team_model_input.unwrap_or_default(),
        card.judgment
    )
}

#[cfg(test)]
mod tests {
    use super::{Confidence, confidence_for_score};
    use crate::model::ConfidenceCalibration;

    #[test]
    fn calibrated_confidence_includes_threshold_boundaries() {
        let calibration = ConfidenceCalibration {
            high_max_normalized_epistemic_score: 4.0,
            medium_max_normalized_epistemic_score: 6.0,
            threshold_training_rows: 100,
            training_period_dates: 20,
            tiers_validated: true,
            minimum_audit_rows_per_tier: 100,
            required_mae_ratio: 0.95,
        };
        assert_eq!(confidence_for_score(3.99, 5, calibration), Confidence::High);
        assert_eq!(confidence_for_score(4.0, 5, calibration), Confidence::High);
        assert_eq!(
            confidence_for_score(4.01, 5, calibration),
            Confidence::Medium
        );
        assert_eq!(
            confidence_for_score(6.0, 5, calibration),
            Confidence::Medium
        );
        assert_eq!(confidence_for_score(6.01, 5, calibration), Confidence::Low);
        assert_eq!(confidence_for_score(3.0, 4, calibration), Confidence::Low);
        assert_eq!(confidence_for_score(3.0, 3, calibration), Confidence::Low);
    }

    #[test]
    fn confidence_score_penalizes_short_history() {
        let calibration = ConfidenceCalibration {
            high_max_normalized_epistemic_score: 4.0,
            medium_max_normalized_epistemic_score: 6.0,
            threshold_training_rows: 100,
            training_period_dates: 20,
            tiers_validated: true,
            minimum_audit_rows_per_tier: 100,
            required_mae_ratio: 0.95,
        };
        assert!(
            calibration.epistemic_score(4.0, 100, 3) > calibration.epistemic_score(4.0, 100, 5)
        );
        assert_eq!(
            calibration.epistemic_score(1.0, 100, 5),
            calibration.epistemic_score(2.0, 25, 5)
        );
    }

    #[test]
    fn unvalidated_tiers_fall_back_without_high_confidence() {
        let calibration = ConfidenceCalibration {
            high_max_normalized_epistemic_score: 4.0,
            medium_max_normalized_epistemic_score: 6.0,
            threshold_training_rows: 100,
            training_period_dates: 20,
            tiers_validated: false,
            minimum_audit_rows_per_tier: 100,
            required_mae_ratio: 0.95,
        };
        assert_eq!(
            confidence_for_score(1.0, 5, calibration),
            Confidence::Medium
        );
        assert_eq!(confidence_for_score(1.0, 4, calibration), Confidence::Low);
        assert_eq!(confidence_for_score(6.01, 5, calibration), Confidence::Low);
        assert_eq!(
            confidence_for_score(f64::NAN, 5, calibration),
            Confidence::Low
        );
        assert_eq!(
            confidence_for_score(f64::INFINITY, 5, calibration),
            Confidence::Low
        );
    }

    #[test]
    fn invalid_thresholds_are_always_low_confidence() {
        let calibration = ConfidenceCalibration {
            high_max_normalized_epistemic_score: 7.0,
            medium_max_normalized_epistemic_score: 6.0,
            threshold_training_rows: 100,
            training_period_dates: 20,
            tiers_validated: true,
            minimum_audit_rows_per_tier: 100,
            required_mae_ratio: 0.95,
        };
        assert_eq!(confidence_for_score(1.0, 5, calibration), Confidence::Low);

        let negative_thresholds = ConfidenceCalibration {
            high_max_normalized_epistemic_score: -2.0,
            medium_max_normalized_epistemic_score: -1.0,
            ..calibration
        };
        assert_eq!(
            confidence_for_score(0.0, 5, negative_thresholds),
            Confidence::Low
        );
    }
}
