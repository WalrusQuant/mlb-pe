import { invoke } from "@tauri-apps/api/core";
import type {
  PredictionsBundle,
  StandingsBundle,
  TeamStatsBundle,
  GameBreakdownBundle,
  GameContextBundle,
} from "./types";

export async function getPredictions(opts: {
  season?: number;
  date?: string;
  exponent?: number;
  includePitchers?: boolean;
  includeHomeField?: boolean;
  includeRecentForm?: boolean;
} = {}): Promise<PredictionsBundle> {
  return await invoke<PredictionsBundle>("get_predictions", opts);
}

export async function getGameBreakdown(opts: {
  season?: number;
  gamePk: number;
  exponent?: number;
  includePitchers?: boolean;
  includeHomeField?: boolean;
  includeRecentForm?: boolean;
}): Promise<GameBreakdownBundle> {
  return await invoke<GameBreakdownBundle>("get_game_breakdown", opts);
}

export async function getGameContext(opts: {
  season?: number;
  gamePk: number;
}): Promise<GameContextBundle> {
  return await invoke<GameContextBundle>("get_game_context", opts);
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

export async function getStandings(season?: number): Promise<StandingsBundle> {
  return await invoke<StandingsBundle>("get_standings", { season });
}
