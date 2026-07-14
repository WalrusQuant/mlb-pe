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
  gamePk: number;
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

// Per-team intermediate values behind a prediction (mirrors Rust SideBreakdown).
// Rate flow: season → (+recent-form blend) → (+pitcher) → effective.
export type SideBreakdown = {
  seasonRsPerGame: number;
  seasonRaPerGame: number;
  recentPresent: boolean;
  recentGames: number;
  recentRsPerGame: number;
  recentRaPerGame: number;
  recentApplied: boolean;
  blendedRsPerGame: number;
  blendedRaPerGame: number;
  pitcherPresent: boolean;
  pitcherEra: number;
  pitcherIp: number;
  pitcherApplied: boolean;
  effectiveRaPerGame: number;
  pythagWinPct: number;
  osEff: number;
  dsEff: number;
  predRuns: number;
};

export type MatchupBreakdown = {
  home: SideBreakdown;
  away: SideBreakdown;
  exponent: number;
  leagueAvgRuns: number;
  neutralHomeWin: number;
  homeFieldApplied: boolean;
  homeFieldDelta: number;
  finalHomeWin: number;
  finalAwayWin: number;
};

export type GameBreakdownBundle = {
  season: number;
  date: string;
  gamePk: number;
  home: string;
  away: string;
  homeTeamId: number;
  awayTeamId: number;
  homePitcher: Pitcher | null;
  awayPitcher: Pitcher | null;
  homeRecent: Recent | null;
  awayRecent: Recent | null;
  prediction: Prediction;
  breakdown: MatchupBreakdown;
};

// ── Tier 2: matchup & team context (mirrors Rust GameContextBundle) ──
export type H2HMeeting = {
  gamePk: number;
  date: string;
  homeName: string;
  awayName: string;
  homeRuns: number;
  awayRuns: number;
};

export type HeadToHead = {
  aId: number;
  bId: number;
  aWins: number;
  bWins: number;
  aRuns: number;
  bRuns: number;
  meetings: H2HMeeting[];
};

export type SplitLine = {
  games: number;
  wins: number;
  losses: number;
  rsPerGame: number;
  raPerGame: number;
};

export type RecentForm = {
  games: number;
  rsPerGame: number;
  raPerGame: number;
};

export type TeamSplits = {
  home: SplitLine;
  road: SplitLine;
  l10: RecentForm | null;
};

export type LineupSpot = {
  order: number;
  name: string;
  position: string;
};

export type Lineups = {
  home: LineupSpot[];
  away: LineupSpot[];
};

export type Bullpen = {
  era: number;
  inningsPitched: number;
  whip: number;
  saves: number;
};

export type GameContextBundle = {
  gamePk: number;
  home: string;
  away: string;
  homeTeamId: number;
  awayTeamId: number;
  headToHead: HeadToHead;
  homeSplits: TeamSplits;
  awaySplits: TeamSplits;
  lineups: Lineups;
  homeBullpen: Bullpen | null;
  awayBullpen: Bullpen | null;
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
  recent_games: number | null;
  recent_rs_per_game: number | null;
  recent_ra_per_game: number | null;
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
  divisionRank: number | null;
  leagueRank: number | null;
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
