// One Monte Carlo of the remaining regular season, then the 12-team bracket.
// Magic number and rest-of-season expectation stay in division_race.rs.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::division_race::{expected_remaining_wins, is_remaining, magic_number, owns_tiebreak};
use crate::mlb_api::{Game, TeamStanding};
use crate::model::{
    compute_recent_form, compute_team_stats, estimate_game_with_pitchers, round_to, TeamStats,
    RECENT_FORM_WINDOW,
};

pub const DEFAULT_N_SIMS: u32 = 10_000;
pub const DEFAULT_SEED: u64 = 1;

const AL: i32 = 103;
const NL: i32 = 104;

// Higher seed hosts every wild-card game. LDS is 2-2-1. LCS and World Series are 2-3-2.
const WC_HOME: [bool; 3] = [true, true, true];
const LDS_HOME: [bool; 5] = [true, true, false, false, true];
const LCS_HOME: [bool; 7] = [true, true, false, false, false, true, true];

const TIEBREAKER_NOTE: &str = "\
Spots are ranked by winning percentage. A tied group is broken by head-to-head among those clubs, \
then intradivision record when they all share a division, then intraleague record. Anything still \
tied is a seeded coin flip, so every division, wild card, pennant, and the World Series is awarded \
and those columns sum to 100%. October uses the same frozen talent as the rest-of-season walk, \
with the series home-field pattern. Pitchers are off.";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WlOverride {
    pub team_id: i32,
    pub wins: i32,
    pub losses: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemainingGame {
    pub game_pk: i64,
    pub date: String,
    pub opponent: String,
    pub opponent_id: i32,
    pub home: bool,
    pub win_prob: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuturesRow {
    pub team_id: i32,
    pub team_name: String,
    pub league_id: i32,
    pub division_id: i32,
    pub wins: i32,
    pub losses: i32,
    pub runs_scored: i32,
    pub runs_allowed: i32,
    pub pythag_win_pct: f64,
    pub magic_number: i32,
    pub magic_vs: String,
    pub clinched: bool,
    pub eliminated: bool,
    pub games_remaining: i32,
    pub expected_remaining_wins: f64,
    pub expected_remaining_losses: f64,
    pub projected_wins: f64,
    pub projected_losses: f64,
    pub p_win_division: f64,
    pub p_wild_card: f64,
    pub p_playoffs: f64,
    pub p_pennant: f64,
    pub p_win_world_series: f64,
    pub remaining: Vec<RemainingGame>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuturesBundle {
    pub season: i32,
    pub last_updated: String,
    pub exponent: f64,
    pub season_games: i32,
    pub n_sims: u32,
    pub seed: u64,
    pub include_home_field: bool,
    pub include_recent_form: bool,
    pub tiebreaker_note: String,
    pub teams: Vec<FuturesRow>,
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn gen_bool(&mut self, p: f64) -> bool {
        if p >= 1.0 {
            return true;
        }
        if p <= 0.0 {
            return false;
        }
        let u = (self.next_u64() as f64) * (1.0 / (u64::MAX as f64));
        u < p
    }
}

struct Club {
    team_id: i32,
    name: String,
    division_id: i32,
    league_id: i32,
    wins: i32,
    losses: i32,
    runs_scored: i32,
    runs_allowed: i32,
}

struct SimState {
    wins: Vec<i32>,
    losses: Vec<i32>,
    div_w: Vec<i32>,
    div_l: Vec<i32>,
    lg_w: Vec<i32>,
    lg_l: Vec<i32>,
    /// h2h[i * n + j] = wins of i against j.
    h2h: Vec<i32>,
}

struct PricedGame {
    home: usize,
    away: usize,
    same_division: bool,
    same_league: bool,
}

struct Ctx<'a> {
    clubs: &'a [Club],
    state: &'a SimState,
    sim_i: u32,
    n: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn build_futures(
    season: i32,
    games: &[Game],
    standings: &[TeamStanding],
    exponent: f64,
    include_home_field: bool,
    include_recent_form: bool,
    n_sims: u32,
    seed: u64,
    wl_overrides: &[WlOverride],
) -> Result<FuturesBundle, String> {
    let n_sims = n_sims.clamp(100, 50_000);
    let clubs = clubs_from_standings(standings, wl_overrides);
    validate_leagues(&clubs)?;
    let n = clubs.len();
    let id_to_idx: HashMap<i32, usize> = clubs
        .iter()
        .enumerate()
        .map(|(i, c)| (c.team_id, i))
        .collect();

    let (stats, lg_avg) = compute_team_stats(games, exponent);
    let stats_by_id: HashMap<i32, &TeamStats> = stats.iter().map(|t| (t.team_id, t)).collect();
    let recent_by_id = if include_recent_form {
        compute_recent_form(games, RECENT_FORM_WINDOW)
    } else {
        HashMap::new()
    };

    let mut p_home = vec![0.5_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            p_home[i * n + j] = match (
                stats_by_id.get(&clubs[i].team_id).copied(),
                stats_by_id.get(&clubs[j].team_id).copied(),
            ) {
                (Some(h), Some(a)) => {
                    let hr = recent_by_id.get(&clubs[i].team_id).copied();
                    let ar = recent_by_id.get(&clubs[j].team_id).copied();
                    estimate_game_with_pitchers(
                        h,
                        a,
                        lg_avg,
                        None,
                        None,
                        hr,
                        ar,
                        exponent,
                        include_home_field,
                    )
                    .home_win_prob
                }
                _ => 0.5,
            };
        }
    }

    let mut base = SimState {
        wins: clubs.iter().map(|c| c.wins).collect(),
        losses: clubs.iter().map(|c| c.losses).collect(),
        div_w: vec![0; n],
        div_l: vec![0; n],
        lg_w: vec![0; n],
        lg_l: vec![0; n],
        h2h: vec![0; n * n],
    };
    // Standings own the W-L. Completed games only seed the tiebreaker splits.
    for g in games.iter().filter(|g| g.is_final()) {
        let (Some(&home), Some(&away)) = (
            id_to_idx.get(&g.home_team_id),
            id_to_idx.get(&g.away_team_id),
        ) else {
            continue;
        };
        let hr = g.home_runs.unwrap_or(0);
        let ar = g.away_runs.unwrap_or(0);
        if hr == ar {
            continue;
        }
        apply_result(
            &mut base,
            n,
            home,
            away,
            hr > ar,
            clubs[home].division_id == clubs[away].division_id,
            clubs[home].league_id == clubs[away].league_id,
            false,
        );
    }

    let mut remaining_by_team: Vec<Vec<RemainingGame>> = vec![Vec::new(); n];
    let mut priced: Vec<PricedGame> = Vec::new();
    for g in games.iter().filter(|g| is_remaining(g)) {
        let (Some(&home), Some(&away)) = (
            id_to_idx.get(&g.home_team_id),
            id_to_idx.get(&g.away_team_id),
        ) else {
            continue;
        };
        let p = p_home[home * n + away];
        remaining_by_team[home].push(RemainingGame {
            game_pk: g.game_pk,
            date: g.date.clone(),
            opponent: clubs[away].name.clone(),
            opponent_id: clubs[away].team_id,
            home: true,
            win_prob: p,
        });
        remaining_by_team[away].push(RemainingGame {
            game_pk: g.game_pk,
            date: g.date.clone(),
            opponent: clubs[home].name.clone(),
            opponent_id: clubs[home].team_id,
            home: false,
            win_prob: (1.0 - p).clamp(0.0, 1.0),
        });
        priced.push(PricedGame {
            home,
            away,
            same_division: clubs[home].division_id == clubs[away].division_id,
            same_league: clubs[home].league_id == clubs[away].league_id,
        });
    }
    for list in &mut remaining_by_team {
        list.sort_by(|a, b| a.date.cmp(&b.date).then(a.game_pk.cmp(&b.game_pk)));
    }

    let counts = simulate(&clubs, &base, &priced, &p_home, n_sims, seed)?;

    let rem_counts: Vec<i32> = remaining_by_team.iter().map(|v| v.len() as i32).collect();
    let g_season = clubs
        .iter()
        .enumerate()
        .map(|(i, c)| c.wins + c.losses + rem_counts[i])
        .max()
        .unwrap_or(162)
        .max(1);

    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let (mn, magic_vs, clinched, eliminated) =
            magic_for(games, &clubs, &rem_counts, g_season, i);
        let rem = std::mem::take(&mut remaining_by_team[i]);
        let probs: Vec<f64> = rem.iter().map(|g| g.win_prob).collect();
        let exp_w = expected_remaining_wins(&probs);
        let exp_l = rem.len() as f64 - exp_w;
        let c = &clubs[i];
        let pythag = stats_by_id
            .get(&c.team_id)
            .map(|s| round_to(s.pythag_win_pct, 4))
            .unwrap_or(0.5);
        let div_p = share(counts.division[i], n_sims);
        let wc_p = share(counts.wild_card[i], n_sims);
        rows.push(FuturesRow {
            team_id: c.team_id,
            team_name: c.name.clone(),
            league_id: c.league_id,
            division_id: c.division_id,
            wins: c.wins,
            losses: c.losses,
            runs_scored: c.runs_scored,
            runs_allowed: c.runs_allowed,
            pythag_win_pct: pythag,
            magic_number: mn,
            magic_vs,
            clinched,
            eliminated,
            games_remaining: rem.len() as i32,
            expected_remaining_wins: round_to(exp_w, 2),
            expected_remaining_losses: round_to(exp_l, 2),
            projected_wins: round_to(c.wins as f64 + exp_w, 2),
            projected_losses: round_to(c.losses as f64 + exp_l, 2),
            p_win_division: div_p,
            p_wild_card: wc_p,
            p_playoffs: share(counts.division[i] + counts.wild_card[i], n_sims),
            p_pennant: share(counts.pennant[i], n_sims),
            p_win_world_series: share(counts.world_series[i], n_sims),
            remaining: rem,
        });
    }
    rows.sort_by(|a, b| {
        a.league_id
            .cmp(&b.league_id)
            .then(a.division_id.cmp(&b.division_id))
            .then(b.wins.cmp(&a.wins))
            .then(a.losses.cmp(&b.losses))
            .then(a.team_name.cmp(&b.team_name))
    });

    Ok(FuturesBundle {
        season,
        last_updated: chrono::Utc::now().to_rfc3339(),
        exponent: round_to(exponent, 4),
        season_games: g_season,
        n_sims,
        seed,
        include_home_field,
        include_recent_form,
        tiebreaker_note: TIEBREAKER_NOTE.to_string(),
        teams: rows,
    })
}

struct Counts {
    division: Vec<u32>,
    wild_card: Vec<u32>,
    pennant: Vec<u32>,
    world_series: Vec<u32>,
}

fn simulate(
    clubs: &[Club],
    base: &SimState,
    games: &[PricedGame],
    p_home: &[f64],
    n_sims: u32,
    seed: u64,
) -> Result<Counts, String> {
    let n = clubs.len();
    let mut counts = Counts {
        division: vec![0; n],
        wild_card: vec![0; n],
        pennant: vec![0; n],
        world_series: vec![0; n],
    };
    let mut rng = XorShift64::new(seed);
    for sim_i in 0..n_sims {
        let mut state = SimState {
            wins: base.wins.clone(),
            losses: base.losses.clone(),
            div_w: base.div_w.clone(),
            div_l: base.div_l.clone(),
            lg_w: base.lg_w.clone(),
            lg_l: base.lg_l.clone(),
            h2h: base.h2h.clone(),
        };
        for g in games {
            let home_won = rng.gen_bool(p_home[g.home * n + g.away]);
            apply_result(
                &mut state,
                n,
                g.home,
                g.away,
                home_won,
                g.same_division,
                g.same_league,
                true,
            );
        }
        let ctx = Ctx {
            clubs,
            state: &state,
            sim_i,
            n,
        };
        let al = seed_league(&ctx, AL)?;
        let nl = seed_league(&ctx, NL)?;
        for (s, team) in al.iter().enumerate() {
            if s < 3 {
                counts.division[*team] += 1;
            } else {
                counts.wild_card[*team] += 1;
            }
        }
        for (s, team) in nl.iter().enumerate() {
            if s < 3 {
                counts.division[*team] += 1;
            } else {
                counts.wild_card[*team] += 1;
            }
        }
        let al_champ = play_league(&mut rng, &al, p_home, n);
        let nl_champ = play_league(&mut rng, &nl, p_home, n);
        counts.pennant[al_champ] += 1;
        counts.pennant[nl_champ] += 1;
        let ws_home = match pct_wl(
            state.wins[al_champ],
            state.losses[al_champ],
            state.wins[nl_champ],
            state.losses[nl_champ],
        ) {
            Ordering::Greater => al_champ,
            Ordering::Less => nl_champ,
            Ordering::Equal => rank_tied(&[al_champ, nl_champ], &ctx)[0],
        };
        let ws_away = if ws_home == al_champ {
            nl_champ
        } else {
            al_champ
        };
        let champ = play_series(&mut rng, ws_home, ws_away, 4, &LCS_HOME, p_home, n);
        counts.world_series[champ] += 1;
    }
    Ok(counts)
}

fn seed_league(ctx: &Ctx, league: i32) -> Result<[usize; 6], String> {
    let divs = division_ids(ctx.clubs, league);
    let mut winners = Vec::with_capacity(3);
    for d in divs {
        let mem = members(ctx.clubs, d);
        let ordered = order_by_record(&mem, ctx);
        winners.push(ordered[0]);
    }
    let ordered_w = order_by_record(&winners, ctx);
    let win_set: HashSet<usize> = winners.iter().copied().collect();
    let rest: Vec<usize> = ctx
        .clubs
        .iter()
        .enumerate()
        .filter(|(i, c)| c.league_id == league && !win_set.contains(i))
        .map(|(i, _)| i)
        .collect();
    if ordered_w.len() < 3 || rest.len() < 3 {
        return Err(format!("league {league} cannot fill a 6-team bracket"));
    }
    let ordered_rest = order_by_record(&rest, ctx);
    let mut seeds = [0usize; 6];
    for (i, t) in ordered_w.into_iter().take(3).enumerate() {
        seeds[i] = t;
    }
    for (i, t) in ordered_rest.into_iter().take(3).enumerate() {
        seeds[3 + i] = t;
    }
    debug_assert_eq!(
        seeds.iter().collect::<HashSet<_>>().len(),
        6,
        "playoff seeds collided"
    );
    Ok(seeds)
}

/// Seeds array is [1, 2, 3, 4, 5, 6]. No reseeding: 1 plays the 4/5 winner, 2 plays the 3/6 winner.
fn lds_sides(
    seeds: &[usize; 6],
    winner_45: usize,
    winner_36: usize,
) -> ((usize, usize), (usize, usize)) {
    ((seeds[0], winner_45), (seeds[1], winner_36))
}

fn play_league(rng: &mut XorShift64, seeds: &[usize; 6], p_home: &[f64], n: usize) -> usize {
    let w36 = play_series(rng, seeds[2], seeds[5], 2, &WC_HOME, p_home, n);
    let w45 = play_series(rng, seeds[3], seeds[4], 2, &WC_HOME, p_home, n);
    let ((s1, opp1), (s2, opp2)) = lds_sides(seeds, w45, w36);
    let lds1 = play_series(rng, s1, opp1, 3, &LDS_HOME, p_home, n);
    let lds2 = play_series(rng, s2, opp2, 3, &LDS_HOME, p_home, n);
    let seed_of = |team: usize| seeds.iter().position(|t| *t == team).unwrap_or(0);
    let (hi, lo) = if seed_of(lds1) < seed_of(lds2) {
        (lds1, lds2)
    } else {
        (lds2, lds1)
    };
    play_series(rng, hi, lo, 4, &LCS_HOME, p_home, n)
}

fn play_series(
    rng: &mut XorShift64,
    higher: usize,
    lower: usize,
    wins_needed: i32,
    higher_home: &[bool],
    p_home: &[f64],
    n: usize,
) -> usize {
    let mut hw = 0;
    let mut lw = 0;
    let mut g = 0;
    while hw < wins_needed && lw < wins_needed {
        let (home, away) = if higher_home[g] {
            (higher, lower)
        } else {
            (lower, higher)
        };
        let home_won = rng.gen_bool(p_home[home * n + away]);
        if home_won == (home == higher) {
            hw += 1;
        } else {
            lw += 1;
        }
        g += 1;
    }
    if hw >= wins_needed {
        higher
    } else {
        lower
    }
}

fn apply_result(
    state: &mut SimState,
    n: usize,
    home: usize,
    away: usize,
    home_won: bool,
    same_division: bool,
    same_league: bool,
    count_wl: bool,
) {
    let (w, l) = if home_won { (home, away) } else { (away, home) };
    if count_wl {
        state.wins[w] += 1;
        state.losses[l] += 1;
    }
    state.h2h[w * n + l] += 1;
    if same_division {
        state.div_w[w] += 1;
        state.div_l[l] += 1;
    }
    if same_league {
        state.lg_w[w] += 1;
        state.lg_l[l] += 1;
    }
}

fn order_by_record(ids: &[usize], ctx: &Ctx) -> Vec<usize> {
    let mut items = ids.to_vec();
    items.sort_by(|a, b| pct_idx(ctx.state, *b, *a).then(a.cmp(b)));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for id in items {
        if let Some(g) = groups.last_mut() {
            if pct_idx(ctx.state, g[0], id) == Ordering::Equal {
                g.push(id);
                continue;
            }
        }
        groups.push(vec![id]);
    }
    let mut out = Vec::with_capacity(ids.len());
    for g in groups {
        if g.len() == 1 {
            out.push(g[0]);
        } else {
            out.extend(rank_tied(&g, ctx));
        }
    }
    out
}

/// Best first. Criteria advance; a subset that pulls ahead is not re-broken on an earlier step.
fn rank_tied(group: &[usize], ctx: &Ctx) -> Vec<usize> {
    fn rec(group: &[usize], crit: u8, ctx: &Ctx) -> Vec<usize> {
        if group.len() <= 1 {
            return group.to_vec();
        }
        if crit >= 3 {
            // Mix in the sim index. A fixed team-id order would hand the same club every coin flip.
            let mut v = group.to_vec();
            v.sort_by_key(|id| (coin_key(ctx.clubs[*id].team_id, ctx.sim_i), *id));
            return v;
        }
        let mut items = group.to_vec();
        items.sort_by(|a, b| criterion_ord(*b, *a, group, crit, ctx).then(a.cmp(b)));
        let mut clusters: Vec<Vec<usize>> = Vec::new();
        for id in items {
            if let Some(c) = clusters.last_mut() {
                if criterion_ord(c[0], id, group, crit, ctx) == Ordering::Equal {
                    c.push(id);
                    continue;
                }
            }
            clusters.push(vec![id]);
        }
        if clusters.len() == 1 {
            return rec(group, crit + 1, ctx);
        }
        let mut out = Vec::with_capacity(group.len());
        for c in clusters {
            if c.len() == 1 {
                out.push(c[0]);
            } else {
                out.extend(rec(&c, crit + 1, ctx));
            }
        }
        out
    }
    rec(group, 0, ctx)
}

fn criterion_ord(a: usize, b: usize, group: &[usize], crit: u8, ctx: &Ctx) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    match crit {
        0 => {
            if !h2h_usable(ctx, group) {
                return Ordering::Equal;
            }
            let (aw, al) = h2h_vs(ctx, a, group);
            let (bw, bl) = h2h_vs(ctx, b, group);
            pct_wl(aw, al, bw, bl)
        }
        1 => {
            if !same_division(ctx, group)
                || !split_usable(group, &ctx.state.div_w, &ctx.state.div_l)
            {
                return Ordering::Equal;
            }
            pct_wl(
                ctx.state.div_w[a],
                ctx.state.div_l[a],
                ctx.state.div_w[b],
                ctx.state.div_l[b],
            )
        }
        _ => {
            if !split_usable(group, &ctx.state.lg_w, &ctx.state.lg_l) {
                return Ordering::Equal;
            }
            pct_wl(
                ctx.state.lg_w[a],
                ctx.state.lg_l[a],
                ctx.state.lg_w[b],
                ctx.state.lg_l[b],
            )
        }
    }
}

fn h2h_vs(ctx: &Ctx, team: usize, group: &[usize]) -> (i32, i32) {
    let mut w = 0;
    let mut l = 0;
    for &other in group {
        if other == team {
            continue;
        }
        w += ctx.state.h2h[team * ctx.n + other];
        l += ctx.state.h2h[other * ctx.n + team];
    }
    (w, l)
}

fn h2h_usable(ctx: &Ctx, group: &[usize]) -> bool {
    group.iter().all(|&id| {
        let (w, l) = h2h_vs(ctx, id, group);
        w + l > 0
    })
}

fn split_usable(group: &[usize], w: &[i32], l: &[i32]) -> bool {
    group.iter().all(|&id| w[id] + l[id] > 0)
}

fn same_division(ctx: &Ctx, group: &[usize]) -> bool {
    let d = ctx.clubs[group[0]].division_id;
    group.iter().all(|&id| ctx.clubs[id].division_id == d)
}

fn pct_idx(state: &SimState, a: usize, b: usize) -> Ordering {
    pct_wl(
        state.wins[a],
        state.losses[a],
        state.wins[b],
        state.losses[b],
    )
}

/// Higher winning percentage is `Greater`. A 0-0 club ranks below any club that has played,
/// so the ordering stays transitive.
fn pct_wl(w1: i32, l1: i32, w2: i32, l2: i32) -> Ordering {
    let g1 = w1 as i64 + l1 as i64;
    let g2 = w2 as i64 + l2 as i64;
    if g1 == 0 && g2 == 0 {
        return Ordering::Equal;
    }
    if g1 == 0 {
        return Ordering::Less;
    }
    if g2 == 0 {
        return Ordering::Greater;
    }
    let left = w1 as i64 * g2;
    let right = w2 as i64 * g1;
    left.cmp(&right)
}

fn coin_key(team_id: i32, sim_i: u32) -> u64 {
    let mut x = (team_id as u64 ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= (sim_i as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 30;
    x
}

fn division_ids(clubs: &[Club], league: i32) -> Vec<i32> {
    let mut d: Vec<i32> = clubs
        .iter()
        .filter(|c| c.league_id == league)
        .map(|c| c.division_id)
        .collect();
    d.sort_unstable();
    d.dedup();
    d
}

fn members(clubs: &[Club], division: i32) -> Vec<usize> {
    clubs
        .iter()
        .enumerate()
        .filter(|(_, c)| c.division_id == division)
        .map(|(i, _)| i)
        .collect()
}

fn clubs_from_standings(standings: &[TeamStanding], overrides: &[WlOverride]) -> Vec<Club> {
    let ov: HashMap<i32, (i32, i32)> = overrides
        .iter()
        .map(|o| (o.team_id, (o.wins.max(0), o.losses.max(0))))
        .collect();
    let mut seen = HashSet::new();
    let mut clubs = Vec::new();
    for t in standings {
        if !seen.insert(t.team_id) {
            continue;
        }
        let (wins, losses) = ov.get(&t.team_id).copied().unwrap_or((t.wins, t.losses));
        clubs.push(Club {
            team_id: t.team_id,
            name: t.team_name.clone(),
            division_id: t.division_id,
            league_id: t.league_id,
            wins,
            losses,
            runs_scored: t.runs_scored,
            runs_allowed: t.runs_allowed,
        });
    }
    clubs
}

fn validate_leagues(clubs: &[Club]) -> Result<(), String> {
    for lg in [AL, NL] {
        let teams: Vec<&Club> = clubs.iter().filter(|c| c.league_id == lg).collect();
        if teams.len() < 6 {
            return Err(format!(
                "league {lg} has {} teams, need at least 6 to fill a bracket",
                teams.len()
            ));
        }
        if division_ids(clubs, lg).len() != 3 {
            return Err(format!("league {lg} does not have 3 divisions"));
        }
    }
    Ok(())
}

fn magic_for(
    games: &[Game],
    clubs: &[Club],
    rem_counts: &[i32],
    g_season: i32,
    i: usize,
) -> (i32, String, bool, bool) {
    let t = &clubs[i];
    let mut mn = 0i32;
    let mut magic_vs = String::new();
    let mut threat_wins = -1i32;
    let mut threat_losses = i32::MAX;
    let mut eliminated = false;
    for (j, u) in clubs.iter().enumerate() {
        if j == i || u.division_id != t.division_id {
            continue;
        }
        let owns = owns_tiebreak(games, t.team_id, u.team_id);
        let m = magic_number(g_season, t.wins, u.losses, owns);
        if m > rem_counts[i] + rem_counts[j] {
            eliminated = true;
        }
        if m > mn
            || (m == mn
                && (u.wins > threat_wins || (u.wins == threat_wins && u.losses < threat_losses)))
        {
            mn = m;
            magic_vs = u.name.clone();
            threat_wins = u.wins;
            threat_losses = u.losses;
        }
    }
    let clinched = mn == 0 && !eliminated;
    (mn, magic_vs, clinched, eliminated)
}

fn share(count: u32, n_sims: u32) -> f64 {
    round_to(count as f64 / n_sims.max(1) as f64, 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mlb_api::GameStatus;

    fn exact_series(wins_needed: i32, higher_home: &[bool], p_at_home: f64, p_on_road: f64) -> f64 {
        fn rec(h: i32, l: i32, g: usize, need: i32, pat: &[bool], ph: f64, pa: f64) -> f64 {
            if h >= need {
                return 1.0;
            }
            if l >= need {
                return 0.0;
            }
            let p = if pat[g] { ph } else { pa };
            p * rec(h + 1, l, g + 1, need, pat, ph, pa)
                + (1.0 - p) * rec(h, l + 1, g + 1, need, pat, ph, pa)
        }
        rec(0, 0, 0, wins_needed, higher_home, p_at_home, p_on_road)
    }

    #[test]
    fn series_probabilities_match_the_bracket_patterns() {
        assert!((exact_series(2, &WC_HOME, 1.0, 1.0) - 1.0).abs() < 1e-12);
        assert!((exact_series(2, &WC_HOME, 0.5, 0.5) - 0.5).abs() < 1e-12);
        // Heterogeneous 2-2-1 (0.7 at home, 0.4 on the road) is not five i.i.d. 0.7 games.
        let lds = exact_series(3, &LDS_HOME, 0.7, 0.4);
        assert!((lds - 0.65548).abs() < 1e-12);
        let iid = exact_series(3, &[true; 5], 0.7, 0.7);
        assert!((iid - 0.83692).abs() < 1e-12);
        assert!(iid - lds > 0.1);
    }

    #[test]
    fn higher_seed_hosts_the_wild_card_series() {
        let n = 2;
        // P(home wins) = 1 for either orientation, so the host sweeps.
        let p = vec![0.0, 1.0, 1.0, 0.0];
        let mut rng = XorShift64::new(1);
        assert_eq!(play_series(&mut rng, 0, 1, 2, &WC_HOME, &p, n), 0);
        assert_eq!(
            play_series(&mut rng, 0, 1, 2, &[false, false, false], &p, n),
            1
        );
    }

    #[test]
    fn no_reseed_one_seed_plays_four_five_winner() {
        let seeds = [10, 20, 30, 40, 50, 60];
        let (top, other) = lds_sides(&seeds, 40, 60);
        assert_eq!(top, (10, 40));
        assert_eq!(other, (20, 60));
    }

    fn club(id: i32, div: i32, lg: i32, w: i32, l: i32) -> Club {
        Club {
            team_id: id,
            name: format!("T{id}"),
            division_id: div,
            league_id: lg,
            wins: w,
            losses: l,
            runs_scored: 100,
            runs_allowed: 100,
        }
    }

    /// 12 clubs, 3 divisions × 2 teams per league. Losses default to 100 so win totals rank the records.
    fn mini(al_wins: [i32; 6], nl_wins: [i32; 6]) -> Vec<Club> {
        let al_div = [201, 201, 202, 202, 200, 200];
        let nl_div = [204, 204, 205, 205, 203, 203];
        let mut clubs = Vec::new();
        for i in 0..6 {
            clubs.push(club(i as i32 + 1, al_div[i], AL, al_wins[i], 100));
        }
        for i in 0..6 {
            clubs.push(club(i as i32 + 7, nl_div[i], NL, nl_wins[i], 100));
        }
        clubs
    }

    fn blank_state(clubs: &[Club]) -> SimState {
        let n = clubs.len();
        SimState {
            wins: clubs.iter().map(|c| c.wins).collect(),
            losses: clubs.iter().map(|c| c.losses).collect(),
            div_w: vec![0; n],
            div_l: vec![0; n],
            lg_w: vec![0; n],
            lg_l: vec![0; n],
            h2h: vec![0; n * n],
        }
    }

    fn flat_p(n: usize, p: f64) -> Vec<f64> {
        let mut v = vec![p; n * n];
        for i in 0..n {
            v[i * n + i] = 0.0;
        }
        v
    }

    #[test]
    fn weaker_division_winner_stays_ahead_of_a_better_wild_card() {
        // AL East winner 80, Central winner 75, West winner 70.
        // Central runner-up has 74, more than the West winner, and is still a wild card.
        let clubs = mini([80, 70, 75, 74, 70, 69], [90, 50, 88, 50, 86, 50]);
        let state = blank_state(&clubs);
        let n = clubs.len();
        let counts = simulate(&clubs, &state, &[], &flat_p(n, 0.5), 100, 1).unwrap();
        let west_winner = 4;
        let central_runner = 3;
        assert_eq!(counts.division[west_winner], 100);
        assert_eq!(counts.wild_card[west_winner], 0);
        assert_eq!(counts.wild_card[central_runner], 100);
        assert_eq!(counts.division[central_runner], 0);
        assert!(clubs[central_runner].wins > clubs[west_winner].wins);
    }

    #[test]
    fn head_to_head_leader_wins_a_tied_division() {
        let clubs = mini([90, 90, 80, 70, 75, 60], [90, 50, 88, 50, 86, 50]);
        let mut state = blank_state(&clubs);
        let n = clubs.len();
        state.h2h[0 * n + 1] = 4;
        state.h2h[1 * n + 0] = 2;
        let counts = simulate(&clubs, &state, &[], &flat_p(n, 0.5), 100, 7).unwrap();
        assert_eq!(counts.division[0], 100);
        assert_eq!(counts.division[1], 0);
    }

    #[test]
    fn interleague_game_moves_both_clubs() {
        // AL East is tied 90-70. Team 0 hosts an NL team and always wins.
        // That NL team was tied with its division mate; the loss drops its winning percentage.
        let clubs = mini([90, 90, 80, 70, 75, 60], [90, 90, 88, 50, 86, 50]);
        let state = blank_state(&clubs);
        let n = clubs.len();
        let mut p = flat_p(n, 0.5);
        p[0 * n + 6] = 1.0;
        let games = vec![PricedGame {
            home: 0,
            away: 6,
            same_division: false,
            same_league: false,
        }];
        let counts = simulate(&clubs, &state, &games, &p, 200, 3).unwrap();
        assert_eq!(counts.division[0], 200, "home club should take the AL East");
        assert_eq!(counts.division[1], 0);
        assert_eq!(
            counts.division[7], 200,
            "NL mate should inherit the division"
        );
        assert_eq!(counts.division[6], 0);
    }

    #[test]
    fn probabilities_conserve_playoff_spots() {
        let clubs = mini([50, 48, 47, 46, 45, 44], [50, 48, 47, 46, 45, 44]);
        let state = blank_state(&clubs);
        let n = clubs.len();
        let games = vec![
            PricedGame {
                home: 0,
                away: 1,
                same_division: true,
                same_league: true,
            },
            PricedGame {
                home: 2,
                away: 8,
                same_division: false,
                same_league: false,
            },
            PricedGame {
                home: 6,
                away: 7,
                same_division: true,
                same_league: true,
            },
        ];
        let n_sims = 2000u32;
        let counts = simulate(&clubs, &state, &games, &flat_p(n, 0.6), n_sims, 11).unwrap();
        let sum = |xs: &[u32]| xs.iter().sum::<u32>();
        assert_eq!(sum(&counts.division), n_sims * 6, "six division flags");
        assert_eq!(sum(&counts.wild_card), n_sims * 6, "six wild cards");
        assert_eq!(sum(&counts.pennant), n_sims * 2, "two pennants");
        assert_eq!(sum(&counts.world_series), n_sims, "one champion");
        // Per league: 3 division titles, 3 wild cards, 1 pennant.
        for league_idxs in [0..6, 6..12] {
            let div: u32 = league_idxs.clone().map(|i| counts.division[i]).sum();
            let wc: u32 = league_idxs.clone().map(|i| counts.wild_card[i]).sum();
            let pen: u32 = league_idxs.clone().map(|i| counts.pennant[i]).sum();
            assert_eq!(div, n_sims * 3);
            assert_eq!(wc, n_sims * 3);
            assert_eq!(pen, n_sims);
        }
    }

    fn fin(pk: i64, date: &str, h: i32, hr: i32, a: i32, ar: i32) -> Game {
        Game {
            game_pk: pk,
            date: date.into(),
            status: GameStatus::Final,
            series_description: Some("Regular Season".into()),
            home_team_id: h,
            home_team_name: format!("T{h}"),
            home_runs: Some(hr),
            away_team_id: a,
            away_team_name: format!("T{a}"),
            away_runs: Some(ar),
            home_pitcher_id: None,
            home_pitcher_name: None,
            away_pitcher_id: None,
            away_pitcher_name: None,
            game_date_time: None,
            venue_id: None,
        }
    }

    fn standing(id: i32, div: i32, lg: i32, w: i32, l: i32, rs: i32, ra: i32) -> TeamStanding {
        TeamStanding {
            team_id: id,
            team_name: format!("T{id}"),
            division_id: div,
            league_id: lg,
            wins: w,
            losses: l,
            pct: String::new(),
            games_back: "-".into(),
            wild_card_rank: None,
            wild_card_games_back: "-".into(),
            division_rank: None,
            league_rank: None,
            runs_scored: rs,
            runs_allowed: ra,
            run_differential: rs - ra,
            streak_code: None,
            last_ten_wins: 0,
            last_ten_losses: 0,
            division_leader: false,
            clinched: false,
        }
    }

    fn full_standings(al: [(i32, i32); 6], nl: [(i32, i32); 6]) -> Vec<TeamStanding> {
        let al_div = [201, 201, 202, 202, 200, 200];
        let nl_div = [204, 204, 205, 205, 203, 203];
        let mut s = Vec::new();
        for i in 0..6 {
            s.push(standing(
                i as i32 + 1,
                al_div[i],
                AL,
                al[i].0,
                al[i].1,
                400,
                400,
            ));
        }
        for i in 0..6 {
            s.push(standing(
                i as i32 + 7,
                nl_div[i],
                NL,
                nl[i].0,
                nl[i].1,
                400,
                400,
            ));
        }
        s
    }

    fn equal_finals() -> Vec<Game> {
        // One equal game per club so Pythagorean exists and sits at .500. Pair within division.
        let pairs = [(1, 2), (3, 4), (5, 6), (7, 8), (9, 10), (11, 12)];
        pairs
            .iter()
            .enumerate()
            .map(|(i, (h, a))| fin(i as i64 + 1, "2026-04-01", *h, 4, *a, 4))
            .collect()
    }

    #[test]
    fn what_if_wins_clinches_without_moving_pythag() {
        let standings = full_standings(
            [(10, 10), (10, 10), (10, 10), (10, 10), (10, 10), (10, 10)],
            [(10, 10), (10, 10), (10, 10), (10, 10), (10, 10), (10, 10)],
        );
        let games = equal_finals();
        let bundle = build_futures(
            2026,
            &games,
            &standings,
            2.0,
            false,
            false,
            100,
            1,
            &[WlOverride {
                team_id: 1,
                wins: 400,
                losses: 0,
            }],
        )
        .unwrap();
        let t = bundle.teams.iter().find(|t| t.team_id == 1).unwrap();
        assert_eq!(t.p_playoffs, 1.0);
        assert!(
            (t.pythag_win_pct - 0.5).abs() < 0.02,
            "pythag {}",
            t.pythag_win_pct
        );
        assert_eq!(t.wins, 400);
    }

    #[test]
    fn decisive_records_are_zero_or_one() {
        let standings = full_standings(
            [(100, 62), (80, 82), (95, 67), (70, 92), (90, 72), (60, 102)],
            [(100, 62), (80, 82), (95, 67), (70, 92), (90, 72), (60, 102)],
        );
        let bundle = build_futures(
            2026,
            &equal_finals(),
            &standings,
            2.0,
            false,
            false,
            100,
            1,
            &[],
        )
        .unwrap();
        for t in &bundle.teams {
            assert!(t.p_win_division == 0.0 || t.p_win_division == 1.0, "{t:?}");
            assert!(t.p_wild_card == 0.0 || t.p_wild_card == 1.0, "{t:?}");
            assert!((t.p_playoffs - (t.p_win_division + t.p_wild_card)).abs() < 1e-9);
        }
        let al_div: f64 = bundle
            .teams
            .iter()
            .filter(|t| t.league_id == AL)
            .map(|t| t.p_win_division)
            .sum();
        assert!((al_div - 3.0).abs() < 1e-9);
        let leader = bundle.teams.iter().find(|t| t.team_id == 1).unwrap();
        assert!(leader.clinched);
        assert_eq!(leader.magic_number, 0);
        assert_eq!(leader.p_win_division, 1.0);
        let last = bundle.teams.iter().find(|t| t.team_id == 6).unwrap();
        assert!(last.eliminated);
        assert_eq!(last.p_win_division, 0.0);
        // Two clubs per division: the three runners-up are the whole wild-card field.
        assert_eq!(last.p_wild_card, 1.0);
        let ws: f64 = bundle.teams.iter().map(|t| t.p_win_world_series).sum();
        // Each share is rounded to 4 decimals, so the column can miss 1.0 by a hair.
        assert!(
            (ws - 1.0).abs() < 0.002,
            "world series column summed to {ws}"
        );
    }

    #[test]
    fn same_seed_reproduces_the_table() {
        let standings = full_standings(
            [(40, 40), (38, 42), (36, 44), (34, 46), (32, 48), (30, 50)],
            [(40, 40), (38, 42), (36, 44), (34, 46), (32, 48), (30, 50)],
        );
        let mut games = equal_finals();
        games.push(Game {
            game_pk: 99,
            date: "2026-09-01".into(),
            status: GameStatus::Preview,
            series_description: Some("Regular Season".into()),
            home_team_id: 1,
            home_team_name: "T1".into(),
            home_runs: None,
            away_team_id: 7,
            away_team_name: "T7".into(),
            away_runs: None,
            home_pitcher_id: None,
            home_pitcher_name: None,
            away_pitcher_id: None,
            away_pitcher_name: None,
            game_date_time: None,
            venue_id: None,
        });
        let a = build_futures(2026, &games, &standings, 2.0, true, false, 300, 99, &[]).unwrap();
        let b = build_futures(2026, &games, &standings, 2.0, true, false, 300, 99, &[]).unwrap();
        for (x, y) in a.teams.iter().zip(b.teams.iter()) {
            assert_eq!(x.p_win_division, y.p_win_division);
            assert_eq!(x.p_wild_card, y.p_wild_card);
            assert_eq!(x.p_pennant, y.p_pennant);
            assert_eq!(x.p_win_world_series, y.p_win_world_series);
        }
    }

    #[test]
    fn completed_series_leader_is_the_division_winner() {
        // Teams 1 and 2 are tied in the standings. Team 1 won the season series 4-2.
        let standings = full_standings(
            [(90, 72), (90, 72), (80, 82), (70, 92), (75, 87), (60, 102)],
            [
                (90, 72),
                (50, 112),
                (88, 74),
                (50, 112),
                (86, 76),
                (50, 112),
            ],
        );
        let mut games = Vec::new();
        for i in 0..4 {
            games.push(fin(i, "2026-05-01", 1, 5, 2, 1));
        }
        for i in 0..2 {
            games.push(fin(10 + i, "2026-06-01", 2, 4, 1, 2));
        }
        games.extend(equal_finals());
        let bundle =
            build_futures(2026, &games, &standings, 2.0, false, false, 100, 5, &[]).unwrap();
        let a = bundle.teams.iter().find(|t| t.team_id == 1).unwrap();
        let b = bundle.teams.iter().find(|t| t.team_id == 2).unwrap();
        assert_eq!(a.p_win_division, 1.0);
        assert_eq!(b.p_win_division, 0.0);
    }
}
