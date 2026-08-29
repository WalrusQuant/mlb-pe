fn main() -> Result<(), Box<dyn std::error::Error>> {
    let date = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2026-07-16".into());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let slate = pns_core::score_date_fixture_detailed(&date)?;
    let path = pns_core::export_predictions_with_model(&slate.cards, Some(&slate.model), &root)?;
    std::fs::write(
        root.join("outputs/validation.json"),
        serde_json::to_string_pretty(&slate.validation)?,
    )?;
    println!(
        "wrote {} cards from {} to {}",
        slate.cards.len(),
        slate.model.artifact_id,
        path.display()
    );
    for card in slate.cards {
        println!(
            "{}: FIP {:.2}, runs {:.2}/{:.2}/{:.2}",
            card.pitcher_name,
            card.projected_fip,
            card.expected_runs_low,
            card.expected_runs_base,
            card.expected_runs_high
        );
    }
    Ok(())
}
