use std::collections::{BTreeSet, HashMap};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("data/processed/training_starts.csv"));
    let output = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| input.clone());
    let seed = args.next().map(std::path::PathBuf::from);
    let mut rows = pns_core::data::dataset::read_csv(&input)?;
    if let Some(seed) = seed {
        let velocities: HashMap<_, _> = pns_core::data::dataset::read_csv(seed)?
            .into_iter()
            .filter_map(|row| {
                row.avg_fb_velo
                    .map(|velocity| ((row.pitcher_id, row.game_pk), velocity))
            })
            .collect();
        for row in &mut rows {
            row.avg_fb_velo = velocities.get(&(row.pitcher_id, row.game_pk)).copied();
        }
    }
    let all_pitcher_ids: Vec<u32> = rows
        .iter()
        .map(|row| row.pitcher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let client = pns_core::data::MlbStatsClient::new()?;

    let mut hands = HashMap::new();
    for chunk in all_pitcher_ids.chunks(100) {
        hands.extend(client.pitcher_hands(chunk)?);
    }
    pns_core::data::enrich::enrich_historical_context(&mut rows, &hands);
    let start = rows
        .iter()
        .filter(|row| row.avg_fb_velo.is_none())
        .map(|row| row.game_date)
        .min()
        .ok_or("dataset has no rows requiring velocity enrichment")?;
    let end = rows
        .iter()
        .map(|row| row.game_date)
        .max()
        .ok_or("dataset is empty")?;
    let mut failures = Vec::new();
    let pitcher_ids: Vec<_> = rows
        .iter()
        .filter(|row| row.avg_fb_velo.is_none())
        .map(|row| row.pitcher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for (chunk_index, chunk) in pitcher_ids.chunks(8).enumerate() {
        eprintln!(
            "velocity enrichment: batch {}/{} ({} pitchers remaining)",
            chunk_index + 1,
            pitcher_ids.len().div_ceil(8),
            pitcher_ids.len().saturating_sub(chunk_index * 8)
        );
        let results: Vec<_> = std::thread::scope(|scope| {
            chunk
                .iter()
                .map(|&pitcher_id| {
                    let client = client.clone();
                    scope.spawn(move || {
                        (
                            pitcher_id,
                            client.savant_fastball_velocity(pitcher_id, start, end),
                        )
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().expect("Savant worker panicked"))
                .collect()
        });
        for (pitcher_id, result) in results {
            match result {
                Ok(velocities) => {
                    pns_core::data::enrich::join_velocity(&mut rows, pitcher_id, &velocities)
                }
                Err(error) => failures.push(format!("{pitcher_id}: {error}")),
            }
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "refusing to write partial enrichment; {} requests failed: {}",
            failures.len(),
            failures.join("; ")
        )
        .into());
    }
    let velocity_rows = rows.iter().filter(|row| row.avg_fb_velo.is_some()).count();
    let coverage = velocity_rows as f64 / rows.len() as f64;
    if coverage < 0.98 {
        return Err(format!(
            "refusing to write enrichment with {:.2}% velocity coverage",
            coverage * 100.0
        )
        .into());
    }
    pns_core::data::dataset::write_csv(&rows, &output)?;
    println!(
        "enriched {} rows through {end}; velocity coverage {:.2}%",
        rows.len(),
        coverage * 100.0
    );
    Ok(())
}
