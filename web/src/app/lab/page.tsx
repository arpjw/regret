"use client";

import { useCallback, useReducer, useRef } from "react";
import {
  fetchConvergence,
  startSolve,
  streamProgress,
  type SolverConfigPayload,
} from "@/lib/api";
import { ConvergenceChart, type Series } from "@/components/ConvergenceChart";

interface Run {
  id: string;
  label: string;
  color: string;
  points: { iteration: number; exploitability: number }[];
  status: "running" | "done" | "error";
}

interface LabState {
  runs: Run[];
}

type Action =
  | { type: "add"; run: Run }
  | { type: "tick"; id: string; iteration: number; exploitability: number }
  | { type: "finish"; id: string; status: "done" | "error" }
  | { type: "remove"; id: string }
  | { type: "replace"; id: string; points: Run["points"] };

function reducer(state: LabState, action: Action): LabState {
  switch (action.type) {
    case "add":
      return { runs: [...state.runs, action.run] };
    case "tick":
      return {
        runs: state.runs.map((r) =>
          r.id === action.id
            ? {
                ...r,
                points: [
                  ...r.points,
                  { iteration: action.iteration, exploitability: action.exploitability },
                ],
              }
            : r,
        ),
      };
    case "finish":
      return {
        runs: state.runs.map((r) => (r.id === action.id ? { ...r, status: action.status } : r)),
      };
    case "remove":
      return { runs: state.runs.filter((r) => r.id !== action.id) };
    case "replace":
      return {
        runs: state.runs.map((r) => (r.id === action.id ? { ...r, points: action.points } : r)),
      };
  }
}

const PALETTE = ["#8b5cf6", "#14b8a6", "#f59e0b", "#3b82f6", "#ef4444", "#a855f7"];

export default function LabPage() {
  const [state, dispatch] = useReducer(reducer, { runs: [] });
  const streams = useRef(new Map<string, EventSource>());

  const launch = useCallback(
    async (label: string, config: SolverConfigPayload) => {
      try {
        const jobId = await startSolve(config);
        const color = PALETTE[state.runs.length % PALETTE.length];
        const run: Run = { id: jobId, label, color, points: [], status: "running" };
        dispatch({ type: "add", run });
        const es = streamProgress(jobId);
        streams.current.set(jobId, es);
        es.onmessage = (msg) => {
          if (!msg.data) return;
          try {
            const evt = JSON.parse(msg.data) as {
              iteration: number;
              exploitability: number;
              done?: boolean;
            };
            dispatch({
              type: "tick",
              id: jobId,
              iteration: evt.iteration,
              exploitability: evt.exploitability,
            });
            if (evt.done) {
              dispatch({ type: "finish", id: jobId, status: "done" });
              es.close();
              streams.current.delete(jobId);
              fetchConvergence(jobId)
                .then((c) => dispatch({ type: "replace", id: jobId, points: c.points }))
                .catch(() => null);
            }
          } catch {
            /* ignore */
          }
        };
        es.onerror = () => {
          dispatch({ type: "finish", id: jobId, status: "error" });
          es.close();
          streams.current.delete(jobId);
        };
      } catch (err) {
        console.error(err);
      }
    },
    [state.runs.length],
  );

  const remove = (id: string) => {
    streams.current.get(id)?.close();
    streams.current.delete(id);
    dispatch({ type: "remove", id });
  };

  const series: Series[] = state.runs.map((r) => ({
    id: r.label,
    color: r.color,
    data: r.points,
  }));

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-line bg-panel p-4">
        <h2 className="mb-3 text-sm font-semibold uppercase tracking-wider text-zinc-400">
          Convergence Lab
        </h2>
        <p className="text-xs text-zinc-500">
          Launch multiple solver configurations and overlay their convergence curves. Each row
          drives an independent solve; the chart updates live as SSE events arrive.
        </p>
        <div className="mt-4 flex flex-wrap gap-2">
          <PresetButton label="50-bucket, 3 sizes" onClick={() => launch("50 / 3", presetA())} />
          <PresetButton label="25-bucket, 3 sizes" onClick={() => launch("25 / 3", presetB())} />
          <PresetButton label="100-bucket, 2 sizes" onClick={() => launch("100 / 2", presetC())} />
        </div>
      </div>

      <ConvergenceChart series={series} height={360} />

      <div className="rounded-lg border border-line bg-panel p-4">
        <h3 className="mb-2 text-sm font-semibold uppercase tracking-wider text-zinc-400">Runs</h3>
        <table className="w-full text-xs">
          <thead className="text-zinc-500">
            <tr>
              <th className="text-left">Label</th>
              <th className="text-left">Job</th>
              <th className="text-left">Samples</th>
              <th className="text-left">Status</th>
              <th className="text-left">Last expl.</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {state.runs.length === 0 && (
              <tr>
                <td colSpan={6} className="py-3 text-center text-zinc-500">
                  No runs yet. Launch a preset above.
                </td>
              </tr>
            )}
            {state.runs.map((r) => {
              const last = r.points[r.points.length - 1];
              return (
                <tr key={r.id} className="border-t border-line">
                  <td className="flex items-center gap-2 py-2">
                    <span
                      className="inline-block h-2 w-2 rounded-sm"
                      style={{ background: r.color }}
                    />
                    {r.label}
                  </td>
                  <td className="font-mono">{r.id}</td>
                  <td>{r.points.length}</td>
                  <td>{r.status}</td>
                  <td className="font-mono">
                    {last ? last.exploitability.toFixed(4) : "—"}
                  </td>
                  <td className="text-right">
                    <button
                      type="button"
                      onClick={() => remove(r.id)}
                      className="rounded-md border border-line px-2 py-1 text-zinc-400 hover:bg-line hover:text-zinc-100"
                    >
                      remove
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function PresetButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md border border-line bg-ink px-3 py-1.5 text-xs text-zinc-300 hover:border-accent-purple hover:text-white"
    >
      + {label}
    </button>
  );
}

function presetA(): SolverConfigPayload {
  return {
    matchup: "BTN_vs_BB",
    stack_depth: 100,
    board: ["Ah", "Ks", "7d"],
    bet_sizes: [0.25, 0.5, 0.75],
    card_buckets: 50,
    max_iterations: 1000,
    starting_pot: 6,
    max_raises_per_street: 3,
  };
}

function presetB(): SolverConfigPayload {
  return { ...presetA(), card_buckets: 25 };
}

function presetC(): SolverConfigPayload {
  return { ...presetA(), card_buckets: 100, bet_sizes: [0.5, 1.0] };
}
