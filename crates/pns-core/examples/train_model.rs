fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dataset = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("data/processed/training_starts.csv"));
    let output = std::env::args()
        .nth(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("models/pns-model-v1.json"));
    let starts = pns_core::data::dataset::read_csv(&dataset)?;
    let artifact = pns_core::model::artifact::ModelArtifact::train(&starts)?;
    let path = artifact.save(&output)?;
    println!(
        "wrote {} from {} normalized starts through {} to {}",
        artifact.artifact_id,
        artifact.training.source_start_count,
        artifact.training.data_through,
        path.display()
    );
    Ok(())
}
