// Types mirror the serde-Serialize structs from src-tauri/src.

export type Prediction = {
  homeWinProb: number;
  awayWinProb: number;
  homeFairOdds: number;
  awayFairOdds: number;
  homePredRuns: number;
  awayPredRuns: number;
  totalRuns: number;
};

export type GameRow = {
  date: string;
  home: string;
  away: string;
} & Prediction;

export type PredictionsBundle = {
  season: number;
  date: string;
  exponent: number;
  leagueAvgRuns: number;
  lastUpdated: string;
  games: GameRow[];
  skipped: string[];
  availableDates: string[];
};

export type TeamStats = {
  team_id: number;
  team: string;
  runs_scored: number;
  runs_allowed: number;
  games_played: number;
  pythag_win_pct: number;
  os: number;
  ds: number;
};

export type TeamStatsBundle = {
  season: number;
  exponent: number;
  leagueAvgRuns: number;
  optimalExponent: number;
  teams: TeamStats[];
};

export type TeamInput = {
  runsScored: number;
  runsAllowed: number;
  games: number;
};
