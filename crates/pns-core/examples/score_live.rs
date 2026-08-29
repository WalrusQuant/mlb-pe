fn main() -> Result<(), Box<dyn std::error::Error>> {
    let date = std::env::args()
        .nth(1)
        .ok_or("usage: cargo run -p pns-core --example score_live -- YYYY-MM-DD")?;
    let slate = pns_core::score_date_detailed(&date)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let export_path =
        pns_core::export_predictions_with_model(&slate.cards, Some(&slate.model), &root)?;
    println!(
        "scored {} MLB probable starters for {date} with {}",
        slate.cards.len(),
        slate.model.artifact_id
    );
    println!("exported {}", export_path.display());
    println!(
        "walk-forward: {} rows across {} dates; FIP MAE {:.3}; runs MAE {:.3}; raw/calibrated 90% run-band coverage {:.1}%/{:.1}% (scale {:.3})",
        slate.validation.held_out_rows,
        slate.validation.held_out_dates,
        slate.validation.fip_mae,
        slate.validation.runs_mae,
        slate.validation.runs_interval_coverage * 100.0,
        slate.validation.calibrated_runs_interval_coverage * 100.0,
        slate.validation.recommended_interval_scale
    );
    println!(
        "FIP baselines — league {:.3}, pitcher history {:.3}, last five {:.3}; model {:.3}",
        slate.validation.league_fip_mae,
        slate.validation.pitcher_history_fip_mae,
        slate.validation.last_five_fip_mae,
        slate.validation.fip_mae
    );
    std::fs::write(
        root.join("outputs/validation.json"),
        serde_json::to_string_pretty(&slate.validation)?,
    )?;
    for card in slate.cards {
        println!(
            "{}: FIP {:.2}, runs {:.2}/{:.2}/{:.2}, {:?}",
            card.pitcher_name,
            card.projected_fip,
            card.expected_runs_low,
            card.expected_runs_base,
            card.expected_runs_high,
            card.confidence
        );
    }
    Ok(())
}
