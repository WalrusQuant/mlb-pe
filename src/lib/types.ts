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

export type Pitcher = {
  name: string;
  era: number;
  inningsPitched: number;
  gamesStarted: number;
  // True when this pitcher's stats actually shifted the prediction.
  // False if the user turned the toggle off OR the sample was too small.
  applied: boolean;
  // True when IP meets the backend's MIN_IP_FOR_ADJUSTMENT threshold.
  // Lets the UI show "(small sample)" without hard-coding the threshold.
  eligibleSample: boolean;
};

export type Recent = {
  games: number;
  rsPerGame: number;
  raPerGame: number;
  // True when the L20 sample actually shifted the prediction.
  // False if the user turned the toggle off OR games < MIN_RECENT_GAMES.
  applied: boolean;
  // True when the L20 sample meets the backend's MIN_RECENT_GAMES threshold.
  eligibleSample: boolean;
};

export type GameRow = {
  date: string;
  home: string;
  away: string;
  homePitcher: Pitcher | null;
  awayPitcher: Pitcher | null;
  homeRecent: Recent | null;
  awayRecent: Recent | null;
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

export type TeamStanding = {
  teamId: number;
  teamName: string;
  divisionId: number;
  leagueId: number;
  wins: number;
  losses: number;
  pct: string;
  gamesBack: string;
  wildCardRank: number | null;
  wildCardGamesBack: string;
  divisionRank: number;
  leagueRank: number;
  runsScored: number;
  runsAllowed: number;
  runDifferential: number;
  streakCode: string | null;
  lastTenWins: number;
  lastTenLosses: number;
  divisionLeader: boolean;
  clinched: boolean;
};

export type StandingsBundle = {
  season: number;
  lastUpdated: string;
  teams: TeamStanding[];
};
