// Magic number and rest-of-season expected wins. The Monte Carlo lives in futures.rs.

use crate::mlb_api::Game;
use crate::model::compute_head_to_head;

// MN = G + 1 - W - L_opp, or G - W - L_opp when this team already owns the
// completed season-series tiebreaker. Floor at 0 (already clinched vs that opponent).
pub fn magic_number(
    season_games: i32,
    wins: i32,
    opponent_losses: i32,
    owns_tiebreak: bool,
) -> i32 {
    let plus = if owns_tiebreak { 0 } else { 1 };
    (season_games + plus - wins - opponent_losses).max(0)
}

// Expected remaining wins is the sum of independent P(win) over remaining games.
pub fn expected_remaining_wins(win_probs: &[f64]) -> f64 {
    win_probs.iter().copied().sum()
}

pub(crate) fn is_remaining(g: &Game) -> bool {
    !g.is_final() && g.status != crate::mlb_api::GameStatus::Other
}

fn remaining_h2h_count(games: &[Game], a: i32, b: i32) -> i32 {
    games
        .iter()
        .filter(|g| is_remaining(g))
        .filter(|g| {
            let ids = (g.home_team_id, g.away_team_id);
            ids == (a, b) || ids == (b, a)
        })
        .count() as i32
}

// True when the season series is finished and `a` leads it, so a final-record
// tie would already belong to `a` on the first tiebreaker.
pub(crate) fn owns_tiebreak(games: &[Game], a: i32, b: i32) -> bool {
    if remaining_h2h_count(games, a, b) > 0 {
        return false;
    }
    let h = compute_head_to_head(games, a, b);
    h.a_wins > h.b_wins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_number_classic() {
        // Leader 90-60 vs pursuer 85-65, G=162 → 162+1-90-65 = 8.
        assert_eq!(magic_number(162, 90, 65, false), 8);
        // Clinched: 100-50 vs 70-80 → 162+1-100-80 = -17 → 0.
        assert_eq!(magic_number(162, 100, 80, false), 0);
    }

    #[test]
    fn magic_number_shrinks_when_tiebreak_owned() {
        // Same records, but leader already owns the completed season series:
        // 162 - 90 - 65 = 7 (the +1 drops).
        assert_eq!(magic_number(162, 90, 65, true), 7);
        assert_eq!(magic_number(162, 90, 65, false), 8);
    }

    #[test]
    fn expected_wins_sums_probs() {
        assert!((expected_remaining_wins(&[0.5, 0.5, 1.0]) - 2.0).abs() < 1e-9);
        assert!((expected_remaining_wins(&[]) - 0.0).abs() < 1e-9);
        assert!((expected_remaining_wins(&[0.25, 0.75]) - 1.0).abs() < 1e-9);
    }
}
