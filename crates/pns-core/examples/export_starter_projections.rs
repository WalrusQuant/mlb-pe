//! Leakage-safe starter projections for the rustyMLB game-model backtest (WP-3, Step A).
//!
//! Fetches every 2026 starter's game log, trains the pns Bayesian linear model on
//! starts through a cutoff (T0), then projects each *post-cutoff* start using only
//! that pitcher's history strictly before the game. Emits one row per starter-start:
//!   game_pk, side (H/A), pitcher_id, expected_runs, expected_innings, sds, starts_used
//!
//! Recent-form core only: park / opponent / velocity are left neutral (the pns
//! standardizer handles the absent velocity feature), so this is a slightly less
//! refined projection than the frozen artifact — fine for a "does the signal help?" test.
//!
//! Usage: cargo run --example export_starter_projections -- [T0=YYYY-MM-DD] [out.csv] [season]

use chrono::NaiveDate;
use pns_core::data::types::{NextStartContext, PitcherStartLog};
use pns_core::data::{dataset, MlbStatsClient};
use pns_core::features::{historical_rows, next_start_row};
use pns_core::model::BayesianLinearModel;
use std::collections::BTreeMap;
use std::path::PathBuf;

const DEFAULT_OUT: &str = "/private/tmp/claude-501/-Users-adamwickwire-GitHub-rustyMLB/e10fc7fa-191d-47c1-b977-bf726b694f1f/scratchpad/starter_projections_2026.csv";
const WINDOW: usize = 5;

#[derive(serde::Serialize)]
struct ProjRow {
    game_pk: u32,
    game_date: String,
    side: &'static str,
    pitcher_id: u32,
    expected_runs: f64,
    expected_innings: f64,
    runs_posterior_sd: f64,
    runs_epistemic_sd: f64,
    starts_used: usize,
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let cutoff = args
        .next()
        .map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()?
        .unwrap_or(NaiveDate::from_ymd_opt(2026, 6, 11).unwrap());
    let output: PathBuf = args.next().unwrap_or_else(|| DEFAULT_OUT.into()).into();
    let season: i32 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(2026);

    let client = MlbStatsClient::new()?;
    eprintln!("fetching all {season} starter logs (min_starts=1) ...");
    let mut all: Vec<PitcherStartLog> = dataset::build_seasons(&client, &[season], 1)?;
    // Chronological within pitcher: features and next_start_row both assume date order.
    all.sort_by(|a, b| {
        a.pitcher_id
            .cmp(&b.pitcher_id)
            .then(a.game_date.cmp(&b.game_date))
            .then(a.game_pk.cmp(&b.game_pk))
    });
    eprintln!("got {} starts from {} pitchers", all.len(), {
        let mut ids: Vec<_> = all.iter().map(|s| s.pitcher_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    });

    // Train coefficients on starts whose target date is on/before the cutoff (leakage-safe
    // for scoring any game after the cutoff).
    let train_starts: Vec<PitcherStartLog> =
        all.iter().filter(|s| s.game_date <= cutoff).cloned().collect();
    let train_rows = historical_rows(&train_starts, WINDOW)?;
    eprintln!(
        "training pns on {} rows (targets <= {cutoff}) ...",
        train_rows.len()
    );
    let model = BayesianLinearModel::fit(&train_rows)?;

    let mut writer = csv::Writer::from_path(&output)?;
    let (mut written, mut skipped) = (0usize, 0usize);
    for s in all.iter().filter(|s| s.game_date > cutoff) {
        let ctx = NextStartContext {
            pitcher_id: s.pitcher_id,
            pitcher_name: s.pitcher_name.clone(),
            game_date: s.game_date,
            opponent_id: s.opponent_id,
            home_team_id: if s.home { s.team_id } else { s.opponent_id },
            home: s.home,
            park_factor: 1.0,
            opponent_offense: 1.0,
            pitcher_team_name: String::new(),
            opponent_team_name: String::new(),
            venue_name: String::new(),
            game_time: None,
            pitcher_hand: None,
        };
        let row = match next_start_row(&all, &ctx, WINDOW) {
            Ok(row) => row,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        match model.score(&row) {
            Ok(p) => {
                writer.serialize(ProjRow {
                    game_pk: s.game_pk,
                    game_date: s.game_date.to_string(),
                    side: if s.home { "H" } else { "A" },
                    pitcher_id: s.pitcher_id,
                    expected_runs: r3(p.expected_runs_base),
                    expected_innings: r3(p.expected_innings),
                    runs_posterior_sd: r3(p.runs_posterior_sd),
                    runs_epistemic_sd: r3(p.runs_epistemic_sd),
                    starts_used: row.starts_used,
                })?;
                written += 1;
            }
            Err(_) => skipped += 1,
        }
    }
    writer.flush()?;
    eprintln!(
        "wrote {written} projections (skipped {skipped}) to {}",
        output.display()
    );

    // --- (b) all-season game -> starters map (needed so the game model can carry s_p
    //         through the training games too, not just the test window) ---
    let dir = output.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut gs: BTreeMap<u32, (Option<u32>, Option<u32>)> = BTreeMap::new();
    for s in &all {
        let e = gs.entry(s.game_pk).or_insert((None, None));
        if s.home {
            e.0 = Some(s.pitcher_id);
        } else {
            e.1 = Some(s.pitcher_id);
        }
    }
    let gs_path = dir.join("game_starters_2026.csv");
    let mut gw = csv::Writer::from_path(&gs_path)?;
    gw.write_record(["game_pk", "home_pitcher_id", "away_pitcher_id"])?;
    for (pk, (h, a)) in &gs {
        gw.write_record([
            pk.to_string(),
            h.map(|v| v.to_string()).unwrap_or_default(),
            a.map(|v| v.to_string()).unwrap_or_default(),
        ])?;
    }
    gw.flush()?;
    eprintln!("wrote {} game-starter rows to {}", gs.len(), gs_path.display());

    // --- (c) leakage-safe per-pitcher prior: project each pitcher "as of the cutoff"
    //         using only starts <= cutoff, so it can seed s_p's prior in the game model ---
    #[derive(serde::Serialize)]
    struct PriorRow {
        pitcher_id: u32,
        prior_er: f64,
        prior_ip: f64,
        epistemic_sd: f64,
        n_starts_through_cutoff: usize,
    }
    let as_of = cutoff + chrono::Duration::days(1); // include the cutoff day's starts
    let mut pitcher_ids: Vec<u32> = all
        .iter()
        .filter(|s| s.game_date <= cutoff)
        .map(|s| s.pitcher_id)
        .collect();
    pitcher_ids.sort_unstable();
    pitcher_ids.dedup();
    let priors_path = dir.join("starter_priors_2026.csv");
    let mut pw = csv::Writer::from_path(&priors_path)?;
    let (mut pn, mut pskip) = (0usize, 0usize);
    let mut prior_rows: Vec<PriorRow> = Vec::new();
    for pid in pitcher_ids {
        let n_prior = all
            .iter()
            .filter(|s| s.pitcher_id == pid && s.game_date <= cutoff)
            .count();
        let ctx = NextStartContext {
            pitcher_id: pid,
            pitcher_name: String::new(),
            game_date: as_of,
            opponent_id: 0,
            home_team_id: 0,
            home: false, // park/opponent/home left neutral: this is a skill prior
            park_factor: 1.0,
            opponent_offense: 1.0,
            pitcher_team_name: String::new(),
            opponent_team_name: String::new(),
            venue_name: String::new(),
            game_time: None,
            pitcher_hand: None,
        };
        match next_start_row(&all, &ctx, WINDOW).and_then(|row| model.score(&row)) {
            Ok(p) => {
                prior_rows.push(PriorRow {
                    pitcher_id: pid,
                    prior_er: r3(p.expected_runs_base),
                    prior_ip: r3(p.expected_innings),
                    epistemic_sd: r3(p.runs_epistemic_sd),
                    n_starts_through_cutoff: n_prior,
                });
                pn += 1;
            }
            Err(_) => pskip += 1,
        }
    }
    for row in &prior_rows {
        pw.serialize(row)?;
    }
    pw.flush()?;
    // JSON alongside the CSV — this is the artifact rustyMLB bundles (include_str!).
    let priors_json = dir.join("starter_priors_2026.json");
    std::fs::write(&priors_json, serde_json::to_string_pretty(&prior_rows)?)?;
    eprintln!(
        "wrote {pn} starter priors (skipped {pskip}) to {} and {}",
        priors_path.display(),
        priors_json.display()
    );
    Ok(())
}
