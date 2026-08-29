// Closing lines from scoresandodds.com. MLB columns are Moneyline / Total / Runline
// (the Desktop Python scraper assumed spread / total / ML).

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::time::Duration;

const BASE: &str = "https://www.scoresandodds.com";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

static HTTP: Lazy<Result<reqwest::Client, String>> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())
});

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosingLine {
    pub date: String,
    pub away: String,
    pub home: String,
    pub away_ml: Option<i32>,
    pub home_ml: Option<i32>,
    pub total: Option<f64>,
    pub away_score: Option<i32>,
    pub home_score: Option<i32>,
}

pub fn american_to_prob(odds: i32) -> f64 {
    if odds < 0 {
        let a = (-odds) as f64;
        a / (a + 100.0)
    } else {
        100.0 / (odds as f64 + 100.0)
    }
}

/// P/L of a 1-unit bet at American odds. Win: +decimal profit. Lose: −1.
pub fn unit_pl(odds: i32, won: bool) -> f64 {
    if !won {
        return -1.0;
    }
    if odds < 0 {
        100.0 / (-odds as f64)
    } else {
        odds as f64 / 100.0
    }
}

/// E[P/L] of a 1-unit bet at `odds` if win probability is `p_win`.
pub fn expected_value(p_win: f64, odds: i32) -> f64 {
    let p = p_win.clamp(0.0, 1.0);
    p * unit_pl(odds, true) - (1.0 - p)
}

/// Side with +EV vs the posted American prices, if any. Juice can make both ≤ 0.
pub fn plus_ev_bet(p_home: f64, home_ml: i32, away_ml: i32) -> Option<(bool, f64)> {
    let eh = expected_value(p_home, home_ml);
    let ea = expected_value(1.0 - p_home, away_ml);
    if eh > 0.0 && eh >= ea {
        Some((true, eh))
    } else if ea > 0.0 {
        Some((false, ea))
    } else {
        None
    }
}

pub fn vig_free_home(home_ml: i32, away_ml: i32) -> f64 {
    let ph = american_to_prob(home_ml);
    let pa = american_to_prob(away_ml);
    let s = ph + pa;
    if s <= 0.0 {
        0.5
    } else {
        ph / s
    }
}

pub fn mlb_nick(full_name: &str) -> &str {
    let n = full_name.trim();
    if n.ends_with("Red Sox") {
        "Red Sox"
    } else if n.ends_with("White Sox") {
        "White Sox"
    } else if n.ends_with("Blue Jays") {
        "Blue Jays"
    } else if n.ends_with("Diamondbacks") || n.ends_with("D-backs") {
        "Diamondbacks"
    } else {
        n.rsplit(' ').next().unwrap_or(n)
    }
}

pub async fn fetch_date(date: &str) -> Result<Vec<ClosingLine>> {
    let url = format!("{BASE}/mlb?date={date}");
    let client = HTTP.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
    let html = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("non-2xx from scoresandodds {date}"))?
        .text()
        .await
        .context("read scoresandodds html")?;
    Ok(parse_mlb_html(&html, date))
}

pub fn parse_mlb_html(html: &str, date: &str) -> Vec<ClosingLine> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(idx) = rest.find("<table") {
        rest = &rest[idx + 6..];
        let Some(end) = rest.find("</table>") else { break };
        let table = &rest[..end];
        rest = &rest[end + 8..];
        if let Some(g) = parse_table(table, date) {
            out.push(g);
        }
    }
    out
}

fn parse_table(table: &str, date: &str) -> Option<ClosingLine> {
    let away = side_chunk(table, "away")?;
    let home = side_chunk(table, "home")?;
    let away_team = team_name(&away)?;
    let home_team = team_name(&home)?;
    if away_team.is_empty() || home_team.is_empty() {
        return None;
    }
    Some(ClosingLine {
        date: date.to_string(),
        away: away_team,
        home: home_team,
        away_ml: parse_american(&attr_value(&away, "live-moneyline")),
        home_ml: parse_american(&attr_value(&home, "live-moneyline")),
        total: parse_total(&attr_value(&away, "live-total"))
            .or_else(|| parse_total(&attr_value(&home, "live-total"))),
        away_score: parse_score(&away),
        home_score: parse_score(&home),
    })
}

fn side_chunk<'a>(table: &'a str, side: &str) -> Option<&'a str> {
    let needle = format!("data-side=\"{side}\"");
    let i = table.find(&needle)?;
    let start = table[..i].rfind("<tr").unwrap_or(i);
    let from = &table[start..];
    let end = from[3..].find("</tr>").map(|e| 3 + e + 5).unwrap_or(from.len());
    Some(&from[..end.min(from.len())])
}

fn team_name(chunk: &str) -> Option<String> {
    let key = "/mlb/teams/";
    let i = chunk.find(key)?;
    let rest = &chunk[i + key.len()..];
    let slug = rest.split(|c: char| c == '"' || c == '\'' || c == '/' || c == ' ').next()?;
    if slug.is_empty() {
        return None;
    }
    Some(slug.replace('-', " "))
        .map(|s| title_case(&s))
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn attr_value(chunk: &str, field: &str) -> String {
    let needle = format!("data-field=\"{field}\"");
    let Some(i) = chunk.find(&needle) else {
        return String::new();
    };
    let rest = &chunk[i..];
    if let Some(v) = data_value(rest) {
        return v;
    }
    String::new()
}

fn data_value(rest: &str) -> Option<String> {
    let i = rest.find("data-value")?;
    let after = rest[i..].find('>')?;
    let inner = &rest[i + after + 1..];
    let end = inner.find('<')?;
    Some(inner[..end].trim().to_string())
}

fn parse_american(raw: &str) -> Option<i32> {
    let t = raw.trim().trim_start_matches('+').replace(',', "");
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("even") || t == "100" {
        return Some(100);
    }
    t.parse::<i32>().ok().filter(|&n| n != 0)
}

fn parse_total(raw: &str) -> Option<f64> {
    let t = raw.trim().trim_start_matches(['o', 'u', 'O', 'U']);
    t.parse::<f64>().ok().filter(|n| *n > 0.0)
}

fn parse_score(chunk: &str) -> Option<i32> {
    let i = chunk.find("event-card-score")?;
    let cell = {
        let rest = &chunk[i..];
        let end = rest.find("</td>").unwrap_or(80.min(rest.len()));
        &rest[..end]
    };
    let digits: String = cell
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<table class="event-card-table">
<thead><tr><th>FINAL</th><th></th><th>Line Movements</th>
<th data-abbr="ML"><span>Moneyline</span></th><th>Total</th>
<th data-abbr="RL"><span>Runline</span></th></tr></thead>
<tbody>
<tr class="event-card-row" data-side="away">
<td><a href="/mlb/teams/reds"><span>Reds</span></a></td>
<td class="event-card-score win">10</td>
<td></td>
<td data-field="live-moneyline" data-side="away"><span class="data-value">+179</span></td>
<td data-field="live-total" data-side="over"><span class="data-value">o9</span></td>
<td data-field="live-spread" data-side="away"><span class="data-value">+1.5</span></td>
</tr>
<tr class="event-card-row" data-side="home">
<td><a href="/mlb/teams/cubs"><span>Cubs</span></a></td>
<td class="event-card-score loss">8</td>
<td></td>
<td data-field="live-moneyline" data-side="home"><span class="data-value">-193</span></td>
<td data-field="live-total" data-side="under"><span class="data-value">u9</span></td>
<td data-field="live-spread" data-side="home"><span class="data-value">-1.5</span></td>
</tr>
</tbody></table>"#;

    #[test]
    fn parse_mlb_moneyline_not_runline() {
        let g = parse_mlb_html(FIXTURE, "2026-08-28");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].away, "Reds");
        assert_eq!(g[0].home, "Cubs");
        assert_eq!(g[0].away_ml, Some(179));
        assert_eq!(g[0].home_ml, Some(-193));
        assert_eq!(g[0].total, Some(9.0));
        assert_eq!(g[0].away_score, Some(10));
        assert_eq!(g[0].home_score, Some(8));
    }

    #[test]
    fn nicknames_match_sao_slugs() {
        assert_eq!(mlb_nick("Cincinnati Reds"), "Reds");
        assert_eq!(mlb_nick("Boston Red Sox"), "Red Sox");
        assert_eq!(mlb_nick("Chicago White Sox"), "White Sox");
        assert_eq!(mlb_nick("Toronto Blue Jays"), "Blue Jays");
        assert_eq!(mlb_nick("Arizona Diamondbacks"), "Diamondbacks");
        assert_eq!(mlb_nick("New York Yankees"), "Yankees");
    }

    #[test]
    fn unit_pl_favorite_and_dog() {
        assert!((unit_pl(-150, true) - 100.0 / 150.0).abs() < 1e-9);
        assert!((unit_pl(150, true) - 1.5).abs() < 1e-9);
        assert!((unit_pl(-150, false) + 1.0).abs() < 1e-9);
        assert!((unit_pl(100, true) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn plus_ev_bets_the_price_not_the_favorite() {
        // Model 55% home, pick'em -110/-110 → home is +EV, away is not.
        let (home, ev) = plus_ev_bet(0.55, -110, -110).unwrap();
        assert!(home);
        assert!(ev > 0.0);
        // Model 55% home, market has home -250 / away +200 → home is -EV.
        let bet = plus_ev_bet(0.55, -250, 200);
        match bet {
            Some((false, ev)) => assert!(ev > 0.0),
            None => {}
            Some((true, _)) => panic!("should not bet the -250 favorite at 55%"),
        }
        assert!(plus_ev_bet(0.52, -150, -150).is_none());
    }

    #[test]
    fn vig_free_favorite_is_above_half() {
        let p = vig_free_home(-193, 179);
        assert!(p > 0.6 && p < 0.72, "cubs -193 vs +179 → {p}");
    }

    #[test]
    fn parse_saved_scoresandodds_page() {
        let Ok(html) = std::fs::read_to_string("/tmp/sao_mlb.html") else {
            return;
        };
        let g = parse_mlb_html(&html, "2026-08-28");
        assert!(g.len() >= 14, "got {}", g.len());
        let cubs = g.iter().find(|x| x.home == "Cubs").expect("cubs");
        assert_eq!(cubs.away, "Reds");
        assert_eq!(cubs.away_ml, Some(179));
        assert_eq!(cubs.home_ml, Some(-193));
        assert_eq!(cubs.total, Some(9.0));
        assert!(g.iter().all(|x| x.home_ml.is_some() && x.away_ml.is_some()));
    }
}
