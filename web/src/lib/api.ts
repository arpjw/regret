export const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

export interface SolverConfigPayload {
  matchup: string;
  stack_depth: number;
  board: string[];
  bet_sizes: number[];
  card_buckets: number;
  max_iterations: number;
  starting_pot: number;
  max_raises_per_street: number;
}

export interface ProgressEvent {
  iteration: number;
  exploitability: number;
  elapsed_ms: number;
  done?: boolean;
}

export interface StrategyResponse {
  infoset: string;
  probabilities: number[];
  job_id?: string;
}

export interface ConvergenceResponse {
  job_id: string;
  points: { iteration: number; exploitability: number; elapsed_ms: number }[];
}

export async function startSolve(payload: SolverConfigPayload): Promise<string> {
  const res = await fetch(`${API_BASE}/solve`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    throw new Error(`POST /solve failed: ${res.status} ${await res.text()}`);
  }
  const body = (await res.json()) as { job_id: string };
  return body.job_id;
}

export function streamProgress(jobId: string): EventSource {
  return new EventSource(`${API_BASE}/solve/${jobId}/stream`);
}

export async function fetchStrategy(
  jobId: string,
  infosetKey: string,
): Promise<StrategyResponse> {
  const url = `${API_BASE}/strategy/${jobId}/${infosetKey}`;
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`GET /strategy failed: ${res.status}`);
  }
  return (await res.json()) as StrategyResponse;
}

export interface HandStrategyResponse {
  job_id: string;
  hand: string;
  hole: [string, string];
  bucket: number;
  history: string;
  player: number;
  infoset_key: number;
  probabilities: number[];
}

/**
 * Resolves a hand label (e.g. "AKs", "AA", "72o") to the engine's infoset
 * key server-side and returns the action distribution stored for it.
 *
 * `history` is a dot-separated list of abstract actions
 * (`check`, `fold`, `call`, `betN`, `raiseN`, `betA`, `raiseA`). Pass an
 * empty string for the start of the flop.
 */
export async function fetchStrategyForHand(
  jobId: string,
  hand: string,
  history: string = "",
): Promise<HandStrategyResponse> {
  const url = new URL(`${API_BASE}/strategy_for_hand/${jobId}/${encodeURIComponent(hand)}`);
  if (history) url.searchParams.set("history", history);
  const res = await fetch(url.toString());
  if (!res.ok) {
    throw new Error(`GET /strategy_for_hand failed: ${res.status}`);
  }
  return (await res.json()) as HandStrategyResponse;
}

export async function fetchConvergence(jobId: string): Promise<ConvergenceResponse> {
  const res = await fetch(`${API_BASE}/convergence/${jobId}`);
  if (!res.ok) {
    throw new Error(`GET /convergence failed: ${res.status}`);
  }
  return (await res.json()) as ConvergenceResponse;
}
