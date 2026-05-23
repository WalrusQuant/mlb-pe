import { invoke } from "@tauri-apps/api/core";
import type {
  PredictionsBundle,
  Prediction,
  TeamStatsBundle,
  TeamInput,
} from "./types";

export async function getPredictions(opts: {
  season?: number;
  date?: string;
  exponent?: number;
  includePitchers?: boolean;
} = {}): Promise<PredictionsBundle> {
  return await invoke<PredictionsBundle>("get_predictions", opts);
}

export async function getTeamStats(opts: {
  season?: number;
  exponent?: number;
} = {}): Promise<TeamStatsBundle> {
  return await invoke<TeamStatsBundle>("get_team_stats", opts);
}

export async function getOptimalExponent(season?: number): Promise<number> {
  return await invoke<number>("get_optimal_exponent", { season });
}

export async function refreshSchedule(season?: number): Promise<number> {
  return await invoke<number>("refresh_schedule", { season });
}

export async function computeMatchup(
  home: TeamInput,
  away: TeamInput,
  exponent: number,
  leagueAvgRuns: number,
): Promise<Prediction> {
  return await invoke<Prediction>("compute_matchup", {
    home,
    away,
    exponent,
    leagueAvgRuns,
  });
}

export async function pythagCurve(
  runsScored: number,
  runsAllowed: number,
  minExp = 0.5,
  maxExp = 4.0,
  steps = 60,
): Promise<[number, number][]> {
  return await invoke<[number, number][]>("pythag_curve", {
    runsScored,
    runsAllowed,
    minExp,
    maxExp,
    steps,
  });
}

export async function americanOdds(prob: number): Promise<number> {
  return await invoke<number>("american_odds", { prob });
}
