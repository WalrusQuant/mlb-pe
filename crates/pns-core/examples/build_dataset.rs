fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seasons = Vec::new();
    let mut output = root.join("data/processed/training_starts.csv");
    let mut through = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().ok_or("--output requires a path")?.into(),
            "--through" => {
                through = Some(chrono::NaiveDate::parse_from_str(
                    &args.next().ok_or("--through requires YYYY-MM-DD")?,
                    "%Y-%m-%d",
                )?)
            }
            value => seasons.push(value.parse::<i32>()?),
        }
    }
    if seasons.len() < 2 {
        return Err("provide at least two seasons, e.g. 2023 2024 2025 2026".into());
    }
    let client = pns_core::data::MlbStatsClient::new()?;
    let mut rows = pns_core::data::dataset::build_seasons(&client, &seasons, 5)?;
    if let Some(cutoff) = through {
        rows.retain(|row| row.game_date <= cutoff);
    }
    if rows.is_empty() {
        return Err("dataset refresh produced no starts".into());
    }
    pns_core::data::dataset::write_csv(&rows, &output)?;
    println!(
        "wrote {} normalized starts through {} to {}",
        rows.len(),
        rows.iter().map(|row| row.game_date).max().unwrap(),
        output.display()
    );
    Ok(())
}
