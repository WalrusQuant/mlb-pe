use super::{BayesianLinearModel, ValidationMetrics};
use crate::data::types::PitcherStartLog;
use crate::features::{self, FEATURE_NAMES, FEATURE_SCHEMA_VERSION};
use crate::{PnsError, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MODEL_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_HISTORY_WINDOW: usize = 5;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureContract {
    pub schema_version: u32,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrainingMetadata {
    pub first_start_date: NaiveDate,
    pub data_through: NaiveDate,
    pub source_start_count: usize,
    pub training_row_count: usize,
    pub minimum_validation_training_rows: usize,
    pub history_window: usize,
    pub normalized_data_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArtifact {
    pub schema_version: u32,
    pub artifact_id: String,
    pub feature_contract: FeatureContract,
    pub training: TrainingMetadata,
    pub model: BayesianLinearModel,
    pub validation: ValidationMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    pub artifact_id: String,
    pub schema_version: u32,
    pub trained_through: NaiveDate,
    pub training_rows: usize,
}

impl ModelArtifact {
    pub fn train(starts: &[PitcherStartLog]) -> Result<Self> {
        if starts.is_empty() {
            return Err(PnsError::Model(
                "cannot train an artifact from an empty normalized dataset".into(),
            ));
        }
        let rows = features::historical_rows(starts, DEFAULT_HISTORY_WINDOW)?;
        if rows.len() < 10 {
            return Err(PnsError::Model(
                "at least ten feature rows are required to train an artifact".into(),
            ));
        }
        let minimum_training_rows = (rows.len() / 2).max(10).min(rows.len() - 1);
        let validation = super::walk_forward_validate(&rows, minimum_training_rows)?;
        let model = BayesianLinearModel::fit(&rows)?
            .with_interval_scale(validation.recommended_interval_scale)
            .with_aux_interval_scales(
                validation.recommended_strikeouts_interval_scale,
                validation.recommended_innings_interval_scale,
            );
        let first_start_date = starts
            .iter()
            .map(|start| start.game_date)
            .min()
            .expect("nonempty starts checked");
        let normalized_data_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(starts)?));
        let data_through = starts
            .iter()
            .map(|start| start.game_date)
            .max()
            .expect("nonempty starts checked");
        let mut artifact = Self {
            schema_version: MODEL_ARTIFACT_SCHEMA_VERSION,
            artifact_id: String::new(),
            feature_contract: FeatureContract {
                schema_version: FEATURE_SCHEMA_VERSION,
                names: FEATURE_NAMES.iter().map(|name| (*name).into()).collect(),
            },
            training: TrainingMetadata {
                first_start_date,
                data_through,
                source_start_count: starts.len(),
                training_row_count: rows.len(),
                minimum_validation_training_rows: minimum_training_rows,
                history_window: DEFAULT_HISTORY_WINDOW,
                normalized_data_sha256,
            },
            model,
            validation,
        };
        artifact.artifact_id = artifact.expected_id()?;
        artifact.validate()?;
        Ok(artifact)
    }

    fn expected_id(&self) -> Result<String> {
        let payload = serde_json::to_vec(&(
            self.schema_version,
            &self.feature_contract,
            &self.training.normalized_data_sha256,
            self.training.data_through,
            self.training.history_window,
        ))?;
        let digest = Sha256::digest(payload);
        let digest_hex = format!("{digest:x}");
        Ok(format!(
            "pns-blr-v1-{}-{}",
            self.training.data_through,
            &digest_hex[..12]
        ))
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MODEL_ARTIFACT_SCHEMA_VERSION {
            return Err(PnsError::Model(format!(
                "unsupported model artifact schema {}; expected {}",
                self.schema_version, MODEL_ARTIFACT_SCHEMA_VERSION
            )));
        }
        let expected_names: Vec<_> = FEATURE_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        if self.feature_contract.schema_version != FEATURE_SCHEMA_VERSION
            || self.feature_contract.names != expected_names
        {
            return Err(PnsError::Model(
                "model artifact feature contract is incompatible with this build".into(),
            ));
        }
        if self.artifact_id != self.expected_id()?
            || self.training.first_start_date > self.training.data_through
            || self.training.source_start_count < self.training.training_row_count
            || self.training.training_row_count < 10
            || self.training.history_window != DEFAULT_HISTORY_WINDOW
            || self.training.minimum_validation_training_rows < 4
            || self.training.minimum_validation_training_rows >= self.training.training_row_count
            || self.training.normalized_data_sha256.len() != 64
            || !self
                .training
                .normalized_data_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(PnsError::Model(
                "model artifact training provenance is invalid".into(),
            ));
        }
        if self.training.training_row_count != self.model.training_row_count {
            return Err(PnsError::Model(
                "model artifact training row counts do not agree".into(),
            ));
        }
        self.validation.validate()?;
        if !self
            .validation
            .confidence_calibration
            .has_valid_thresholds()
        {
            return Err(PnsError::Model(
                "model artifact confidence calibration is invalid".into(),
            ));
        }
        self.model.validate(FEATURE_NAMES.len())
    }

    pub fn info(&self) -> ModelInfo {
        ModelInfo {
            artifact_id: self.artifact_id.clone(),
            schema_version: self.schema_version,
            trained_through: self.training.data_through,
            training_rows: self.training.training_row_count,
        }
    }

    pub fn score(&self, row: &features::FeatureRow) -> Result<crate::Projection> {
        self.validate()?;
        if row.as_of <= self.training.data_through {
            return Err(PnsError::Model(format!(
                "model artifact {} was trained through {}; refusing to score {} because that would leak future data",
                self.artifact_id, self.training.data_through, row.as_of
            )));
        }
        self.model.score(row)
    }

    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        Ok(format!("{}\n", serde_json::to_string_pretty(self)?))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let artifact: Self = serde_json::from_str(json)?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let json = std::fs::read_to_string(path).map_err(|error| {
            PnsError::Model(format!(
                "could not read model artifact {}: {error}",
                path.display()
            ))
        })?;
        Self::from_json(&json)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        let parent = path.parent().ok_or_else(|| {
            PnsError::Model(format!(
                "model artifact path {} has no parent",
                path.display()
            ))
        })?;
        std::fs::create_dir_all(parent)?;
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            path.with_extension(format!("json.{}.{}.tmp", std::process::id(), sequence));
        std::fs::write(&temporary, self.to_json_pretty()?)?;
        if let Err(error) = std::fs::rename(&temporary, path) {
            if !path.exists() {
                std::fs::remove_file(&temporary).ok();
                return Err(error.into());
            }
            let backup =
                path.with_extension(format!("json.{}.{}.old", std::process::id(), sequence));
            std::fs::rename(path, &backup)?;
            if let Err(replace_error) = std::fs::rename(&temporary, path) {
                std::fs::rename(&backup, path).ok();
                std::fs::remove_file(&temporary).ok();
                return Err(replace_error.into());
            }
            std::fs::remove_file(backup)?;
        }
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn artifact_round_trip_is_deterministic_and_preserves_scores() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let artifact = super::ModelArtifact::train(&starts).unwrap();
        let json = artifact.to_json_pretty().unwrap();
        let loaded = super::ModelArtifact::from_json(&json).unwrap();
        assert_eq!(
            json,
            super::ModelArtifact::train(&starts)
                .unwrap()
                .to_json_pretty()
                .unwrap()
        );

        let mut row = crate::features::historical_rows(&starts, 5).unwrap()[0].clone();
        row.as_of = artifact.training.data_through + chrono::Duration::days(1);
        let before = artifact.score(&row).unwrap();
        let after = loaded.score(&row).unwrap();
        assert!((before.projected_fip - after.projected_fip).abs() < 1e-12);
        assert!((before.expected_runs_base - after.expected_runs_base).abs() < 1e-12);
        assert!((before.expected_strikeouts - after.expected_strikeouts).abs() < 1e-12);
        assert!((before.expected_innings - after.expected_innings).abs() < 1e-12);
    }

    #[test]
    fn artifact_rejects_schema_and_feature_contract_changes() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let mut artifact = super::ModelArtifact::train(&starts).unwrap();
        artifact.schema_version += 1;
        assert!(artifact.validate().is_err());

        artifact.schema_version = super::MODEL_ARTIFACT_SCHEMA_VERSION;
        artifact.feature_contract.names.swap(0, 1);
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn artifact_rejects_invalid_model_dimensions() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let mut artifact = super::ModelArtifact::train(&starts).unwrap();
        artifact.model.feature_scales.pop();
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn artifact_load_reports_a_missing_file_as_a_model_error() {
        let path = std::env::temp_dir().join("pns-model-that-does-not-exist.json");
        let error = super::ModelArtifact::load(&path).unwrap_err().to_string();
        assert!(error.contains("could not read model artifact"));
        assert!(error.contains(&path.display().to_string()));
    }

    #[test]
    fn artifact_refuses_to_score_on_or_before_its_training_cutoff() {
        let starts = crate::data::pitcher_logs::fixture_starts().unwrap();
        let artifact = super::ModelArtifact::train(&starts).unwrap();
        let mut row = crate::features::historical_rows(&starts, 5)
            .unwrap()
            .pop()
            .unwrap();
        row.as_of = artifact.training.data_through;
        assert!(
            artifact
                .score(&row)
                .unwrap_err()
                .to_string()
                .contains("leak")
        );
    }
}
