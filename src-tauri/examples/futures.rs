// Live futures table. cargo run --example futures
// Optional: cargo run --example futures -- 2026 2000

use std::time::Instant;

use mlbpe_lib::futures::{build_futures, DEFAULT_N_SIMS, DEFAULT_SEED};
use mlbpe_lib::mlb_api::{fetch_schedule, fetch_standings};
use mlbpe_lib::model::optimize_exponent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let season: i32 = std::env::args()
        .nth(1)
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            chrono::Utc::now()
                .date_naive()
                .format("%Y")
                .to_string()
                .parse()
                .unwrap_or(2026)
        });
    let n_sims: u32 = std::env::args()
        .nth(2)
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_N_SIMS);

    eprintln!("Fetching {season} schedule and standings...");
    let games = fetch_schedule(season).await?;
    let standings = fetch_standings(season).await?;
    let exp = optimize_exponent(&games);
    eprintln!("Exponent {exp:.4}. Simulating {n_sims} seasons...");

    let started = Instant::now();
    let bundle = build_futures(
        season,
        &games,
        &standings,
        exp,
        true,
        true,
        n_sims,
        DEFAULT_SEED,
        &[],
    )?;
    eprintln!("Sim finished in {:.2}s", started.elapsed().as_secs_f64());

    let mut teams = bundle.teams.clone();
    teams.sort_by(|a, b| {
        a.league_id.cmp(&b.league_id).then(
            b.p_win_world_series
                .partial_cmp(&a.p_win_world_series)
                .unwrap(),
        )
    });

    println!(
        "\n{:<22} {:>5} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "Team", "Div", "WC", "Playoff", "Pennant", "WS", "W", "L", "Pythag"
    );
    let mut league = 0;
    for t in &teams {
        if t.league_id != league {
            league = t.league_id;
            println!(
                "\n{}",
                if league == 103 {
                    "American League"
                } else {
                    "National League"
                }
            );
        }
        println!(
            "{:<22} {:>5.1}% {:>5.1}% {:>6.1}% {:>6.1}% {:>6.1}% {:>4} {:>4} {:>6.1}%",
            t.team_name,
            t.p_win_division * 100.0,
            t.p_wild_card * 100.0,
            t.p_playoffs * 100.0,
            t.p_pennant * 100.0,
            t.p_win_world_series * 100.0,
            t.wins,
            t.losses,
            t.pythag_win_pct * 100.0,
        );
    }

    let ws: f64 = bundle.teams.iter().map(|t| t.p_win_world_series).sum();
    let al_pen: f64 = bundle
        .teams
        .iter()
        .filter(|t| t.league_id == 103)
        .map(|t| t.p_pennant)
        .sum();
    let nl_pen: f64 = bundle
        .teams
        .iter()
        .filter(|t| t.league_id == 104)
        .map(|t| t.p_pennant)
        .sum();
    eprintln!(
        "\nColumn sums — AL pennant {al_pen:.4}, NL pennant {nl_pen:.4}, World Series {ws:.4}"
    );
    Ok(())
}
