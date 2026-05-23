// End-to-end smoke test: hits the live MLB API and runs the model.
// Run with:  cargo run --example smoke

use mlbpe_lib::mlb_api::{fetch_pitcher_stats, fetch_schedule};
use mlbpe_lib::model::{
    compute_team_stats, estimate_game, estimate_game_with_pitchers, optimize_exponent, PitcherAdj,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let season: i32 = std::env::args()
        .nth(1)
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive().format("%Y").to_string().parse().unwrap_or(2025));

    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let target = std::env::args().nth(2).unwrap_or(today.clone());

    eprintln!("Season: {}", season);
    eprintln!("Target date: {}", target);

    eprintln!("Fetching schedule...");
    let games = fetch_schedule(season).await?;
    eprintln!("Got {} regular-season games", games.len());
    let finals = games.iter().filter(|g| g.is_final()).count();
    eprintln!("  → {} final, {} scheduled-or-other", finals, games.len() - finals);

    eprintln!("Optimizing exponent...");
    let exp = optimize_exponent(&games);
    eprintln!("Optimal exponent: {:.4}", exp);

    let (stats, lg_avg) = compute_team_stats(&games, exp);
    eprintln!("Team stats: {} teams; league avg runs/team/game = {:.3}", stats.len(), lg_avg);

    let by_id: std::collections::HashMap<i32, &mlbpe_lib::model::TeamStats> =
        stats.iter().map(|t| (t.team_id, t)).collect();

    let today_games: Vec<_> = games.iter().filter(|g| g.date == target).collect();
    eprintln!("\nGames on {}: {}", target, today_games.len());

    // Fetch probable pitcher stats for today's games.
    let pitcher_ids: Vec<i32> = today_games
        .iter()
        .flat_map(|g| [g.home_pitcher_id, g.away_pitcher_id])
        .flatten()
        .collect();
    let mut unique_ids = pitcher_ids.clone();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    eprintln!("Fetching stats for {} probable pitcher(s)...", unique_ids.len());
    let pitchers = fetch_pitcher_stats(season, &unique_ids).await.unwrap_or_default();

    for g in &today_games {
        let home = by_id.get(&g.home_team_id);
        let away = by_id.get(&g.away_team_id);
        match (home, away) {
            (Some(h), Some(a)) => {
                let home_p = g
                    .home_pitcher_id
                    .and_then(|id| pitchers.get(&id))
                    .map(|p| PitcherAdj { era: p.era, innings_pitched: p.innings_pitched });
                let away_p = g
                    .away_pitcher_id
                    .and_then(|id| pitchers.get(&id))
                    .map(|p| PitcherAdj { era: p.era, innings_pitched: p.innings_pitched });
                let base = estimate_game(h, a, lg_avg);
                let p = estimate_game_with_pitchers(h, a, lg_avg, home_p, away_p, exp);
                let hp_str = g
                    .home_pitcher_name
                    .as_deref()
                    .map(|n| {
                        let era = g
                            .home_pitcher_id
                            .and_then(|id| pitchers.get(&id))
                            .map(|x| format!("{:.2}", x.era))
                            .unwrap_or_else(|| "—".into());
                        format!("{} ({})", n, era)
                    })
                    .unwrap_or_else(|| "TBD".into());
                let ap_str = g
                    .away_pitcher_name
                    .as_deref()
                    .map(|n| {
                        let era = g
                            .away_pitcher_id
                            .and_then(|id| pitchers.get(&id))
                            .map(|x| format!("{:.2}", x.era))
                            .unwrap_or_else(|| "—".into());
                        format!("{} ({})", n, era)
                    })
                    .unwrap_or_else(|| "TBD".into());
                println!(
                    "{:>22} @ {:<22}  base {:.2} → adj {:.2}  Δ {:+.2}  | {} vs {}",
                    g.away_team_name,
                    g.home_team_name,
                    base.home_win_prob,
                    p.home_win_prob,
                    p.home_win_prob - base.home_win_prob,
                    ap_str,
                    hp_str,
                );
            }
            _ => {
                println!(
                    "{:>22} @ {:<22}  (skipped — missing stats)",
                    g.away_team_name, g.home_team_name
                );
            }
        }
    }

    Ok(())
}
