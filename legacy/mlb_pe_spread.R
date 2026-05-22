# Predicting MLB game outcomes using Pythagorean Expectation.
# Adam Wickwire - Bettor Analysis

# Outputs a formatted table with Date, Home Team, Away Team, Home Win Probability (0-1, >0.5 favors home team),
# Home Predicted Runs, Away Predicted Runs, and Total Runs (sum of predicted runs).
# Data is sourced from the baseballr package, using team run differentials for predictions.

library(tidyverse)
library(baseballr)
library(lubridate)
library(knitr)

# Fetch and prepare MLB schedule data for the 2025 season
mlb_schedule <- mlb_schedule(season = 2025)
data <- mlb_schedule %>%
  filter(series_description == "Regular Season") %>%
  select(date, teams_away_team_name, teams_away_score, teams_home_team_name, teams_home_score) %>%
  rename(away_team = teams_away_team_name, away_runs = teams_away_score, 
         home_team = teams_home_team_name, home_runs = teams_home_score) %>%
  mutate(date = as.Date(date, format = "%Y-%m-%d"))

# Split data into past games (completed) and today's games (to predict)
today <- Sys.Date()
tomorrow <- today + 1
past_games <- data %>% filter(!is.na(home_runs))
current_schedule <- mlb_schedule %>%
  filter(series_description == "Regular Season", date == today) %>%  # change to tomorrow for next day's games
  select(date, teams_away_team_name, teams_home_team_name) %>%
  rename(away = teams_away_team_name, home = teams_home_team_name) %>%
  mutate(date = as.Date(date, format = "%Y-%m-%d"))

# Calculate team statistics: total runs scored, runs allowed, and games played
home_stats <- past_games %>%
  group_by(home_team) %>%
  summarise(runs_scored_home = sum(home_runs, na.rm = TRUE), 
            runs_allowed_home = sum(away_runs, na.rm = TRUE), games_home = n())
away_stats <- past_games %>%
  group_by(away_team) %>%
  summarise(runs_scored_away = sum(away_runs, na.rm = TRUE), 
            runs_allowed_away = sum(home_runs, na.rm = TRUE), games_away = n())
team_stats <- full_join(home_stats, away_stats, by = c("home_team" = "away_team")) %>%
  mutate(team = home_team, runs_scored = runs_scored_home + runs_scored_away, 
         runs_allowed = runs_allowed_home + runs_allowed_away, games_played = games_home + games_away) %>%
  select(team, runs_scored, runs_allowed, games_played)

# Compute Pythagorean expected win percentage and league-average runs
team_stats <- team_stats %>%
  mutate(pythag_win_pct = runs_scored^2 / (runs_scored^2 + runs_allowed^2))
total_runs <- sum(past_games$home_runs, na.rm = TRUE) + sum(past_games$away_runs, na.rm = TRUE)
total_games <- nrow(past_games)
league_avg_runs <- total_runs / (2 * total_games)

# Calculate offensive (OS) and defensive (DS) strengths relative to league average
team_stats <- team_stats %>%
  mutate(OS = runs_scored / games_played / league_avg_runs, DS = runs_allowed / games_played / league_avg_runs)

# Define log5 function for home team win probability
log5 <- function(p_a, p_b) {
  (p_a * (1 - p_b)) / (p_a * (1 - p_b) + (1 - p_a) * p_b)
}

# Estimate game outcomes: home win probability, predicted runs, and total runs
estimate_game <- function(home_team, away_team, team_stats, league_avg_runs) {
  home_data <- team_stats %>% filter(team == home_team)
  away_data <- team_stats %>% filter(team == away_team)
  if(nrow(home_data) == 0 || nrow(away_data) == 0) return(NULL)
  expected_home_runs <- home_data$OS * away_data$DS * league_avg_runs
  expected_away_runs <- away_data$OS * home_data$DS * league_avg_runs
  estimated_total <- expected_home_runs + expected_away_runs
  win_prob <- log5(home_data$pythag_win_pct, away_data$pythag_win_pct)
  return(list(
    win_prob = round(win_prob, 2),
    home_runs = round(expected_home_runs, 1),
    away_runs = round(expected_away_runs, 1),
    total_runs = round(estimated_total, 1)
  ))
}


# Function to convert probability to American odds
prob_to_american_odds <- function(prob) {
  if (prob <= 0 || prob >= 1) return(NA)
  if (prob > 0.5) {
    return(round(-100 * prob / (1 - prob)))
  } else {
    return(round((1 - prob) * 100 / prob))
  }
}


# Generate and display predictions for today's games in a formatted table
if(nrow(current_schedule) == 0) {
  cat("No games scheduled for today.\n")
} else {
  predictions <- list()
  for(i in 1:nrow(current_schedule)) {
    home_team <- current_schedule$home[i]
    away_team <- current_schedule$away[i]
    pred <- estimate_game(home_team, away_team, team_stats, league_avg_runs)
    if(is.null(pred)) {
      cat("Skipping game: ", home_team, " vs ", away_team, " - team not found.\n")
      next
    }
    predictions[[i]] <- data.frame(
      Date = current_schedule$date[i],
      Home = home_team,
      Away = away_team,
      Home_Win_Probability = pred$win_prob,
      Home_Fair_Odds = prob_to_american_odds(pred$win_prob),
      Away_Win_Probability = 1 - pred$win_prob,
      Away_Fair_Odds = prob_to_american_odds(1 - pred$win_prob),
      Home_Predicted_Runs = pred$home_runs,
      Away_Predicted_Runs = pred$away_runs,
      Total_Runs = pred$total_runs
    )
  }
  if(length(predictions) > 0) {
    predictions_df <- do.call(rbind, predictions)
    cat("\nPredictions for Today's MLB Games:\n")
    print(kable(predictions_df, format = "simple", align = "l",
                col.names = c("Date", "Home Team", "Away Team", 
                              "Home Win Prob", "Home Fair Odds", 
                              "Away Win Prob", "Away Fair Odds",
                              "Home Pred Runs", "Away Pred Runs", "Total Runs")))
  } else {
    cat("No predictions generated due to missing data.\n")
  }
}

# Write to a CSV file
write.csv(predictions_df, "mlb_predictions.csv", row.names = FALSE)


# write to a csv file
write.csv(predictions_df, "mlb_predictions.csv", row.names = FALSE)




