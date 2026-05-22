// End-to-end smoke test: hits the live MLB API and runs the model.
// Run with:  cargo run --example smoke

use mlbpe_lib::mlb_api::fetch_schedule;
use mlbpe_lib::model::{compute_team_stats, estimate_game, optimize_exponent};

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

    // Verify JSON shape — first game's serialized form is what the frontend sees.
    if let Some(g) = today_games.first() {
        if let (Some(h), Some(a)) = (by_id.get(&g.home_team_id), by_id.get(&g.away_team_id)) {
            let p = estimate_game(h, a, lg_avg);
            let row = mlbpe_lib::model::GameRow {
                date: g.date.clone(),
                home: g.home_team_name.clone(),
                away: g.away_team_name.clone(),
                pred: p,
            };
            eprintln!(
                "\nSample serialized GameRow:\n{}\n",
                serde_json::to_string_pretty(&row)?
            );
        }
    }

    for g in &today_games {
        let home = by_id.get(&g.home_team_id);
        let away = by_id.get(&g.away_team_id);
        match (home, away) {
            (Some(h), Some(a)) => {
                let p = estimate_game(h, a, lg_avg);
                println!(
                    "{:>22} @ {:<22}  home {:.2} ({:+}) | away {:.2} ({:+}) | {:.1}–{:.1} (tot {:.1})",
                    g.away_team_name,
                    g.home_team_name,
                    p.home_win_prob,
                    p.home_fair_odds,
                    p.away_win_prob,
                    p.away_fair_odds,
                    p.home_pred_runs,
                    p.away_pred_runs,
                    p.total_runs,
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
