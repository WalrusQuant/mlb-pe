use super::{MlbStatsClient, types::PitcherStartLog};
use crate::Result;
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

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
struct Split {
    player: Player,
    stat: SeasonStat,
}
#[derive(Deserialize)]
struct Player {
    id: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonStat {
    #[serde(default)]
    games_started: u16,
}

pub fn normalize_season_starters(json: &str, minimum_starts: u16) -> Result<Vec<u32>> {
    let response: StatsResponse = serde_json::from_str(json)?;
    let mut ids: Vec<_> = response
        .stats
        .into_iter()
        .flat_map(|group| group.splits)
        .filter(|split| split.stat.games_started >= minimum_starts)
        .map(|split| split.player.id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub fn build_seasons(
    client: &MlbStatsClient,
    seasons: &[i32],
    minimum_starts: u16,
) -> Result<Vec<PitcherStartLog>> {
    let mut rows = Vec::new();
    for &season in seasons {
        let ids = client.season_starters(season, minimum_starts)?;
        let start = NaiveDate::from_ymd_opt(season, 1, 1).expect("valid season start");
        let end = NaiveDate::from_ymd_opt(season, 12, 31).expect("valid season end");
        for chunk in ids.chunks(12) {
            let results: Vec<_> = std::thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|&pitcher_id| {
                        let client = client.clone();
                        scope.spawn(move || client.pitcher_starts_range(pitcher_id, start, end))
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| handle.join().expect("dataset worker panicked"))
                    .collect()
            });
            for pitcher_rows in results {
                rows.extend(pitcher_rows?);
            }
        }
    }
    let deduplicated: BTreeMap<_, _> = rows
        .into_iter()
        .map(|row| ((row.pitcher_id, row.game_pk), row))
        .collect();
    Ok(deduplicated.into_values().collect())
}

pub fn write_csv(rows: &[PitcherStartLog], path: impl AsRef<Path>) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = csv::Writer::from_path(path)?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_csv(path: impl AsRef<Path>) -> Result<Vec<PitcherStartLog>> {
    let mut reader = csv::Reader::from_path(path)?;
    reader
        .deserialize()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn read_csv_text(text: &str) -> Result<Vec<PitcherStartLog>> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    reader
        .deserialize()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    #[test]
    fn filters_season_inventory_to_real_starters() {
        let json = r#"{"stats":[{"splits":[{"player":{"id":3},"stat":{"gamesStarted":12}},{"player":{"id":2},"stat":{"gamesStarted":4}},{"player":{"id":1},"stat":{"gamesStarted":20}}]}]}"#;
        assert_eq!(
            super::normalize_season_starters(json, 5).unwrap(),
            vec![1, 3]
        );
    }

    #[test]
    fn normalized_csv_round_trips() {
        let rows = crate::data::pitcher_logs::fixture_starts().unwrap();
        let path = std::env::temp_dir().join("pns-training-roundtrip.csv");
        super::write_csv(&rows, &path).unwrap();
        let loaded = super::read_csv(path).unwrap();
        assert_eq!(loaded.len(), rows.len());
        assert_eq!(loaded[0].game_pk, rows[0].game_pk);
    }
}
