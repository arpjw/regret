"use client";

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import {
  fetchStrategyForHand,
  startSolve,
  streamProgress,
  type ProgressEvent,
  type SolverConfigPayload,
} from "@/lib/api";
import { SolverConfig } from "@/components/SolverConfig";
import { ConvergenceChart } from "@/components/ConvergenceChart";
import { RangeHeatmap, type CellStrategy } from "@/components/RangeHeatmap";
import { ActionDistribution } from "@/components/ActionDistribution";

type Status = "idle" | "running" | "done" | "error";

interface SolveState {
  status: Status;
  jobId: string | null;
  error: string | null;
  points: { iteration: number; exploitability: number }[];
  config: SolverConfigPayload | null;
}

type Action =
  | { type: "start"; jobId: string; config: SolverConfigPayload }
  | { type: "progress"; evt: ProgressEvent }
  | { type: "done" }
  | { type: "error"; message: string }
  | { type: "reset" };

const initialState: SolveState = {
  status: "idle",
  jobId: null,
  error: null,
  points: [],
  config: null,
};

function reducer(state: SolveState, action: Action): SolveState {
  switch (action.type) {
    case "start":
      return {
        status: "running",
        jobId: action.jobId,
        error: null,
        points: [],
        config: action.config,
      };
    case "progress":
      return {
        ...state,
        points: [
          ...state.points,
          { iteration: action.evt.iteration, exploitability: action.evt.exploitability },
        ],
      };
    case "done":
      return { ...state, status: "done" };
    case "error":
      return { ...state, status: "error", error: action.message };
    case "reset":
      return initialState;
  }
}

export default function SolverPage() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const eventSourceRef = useRef<EventSource | null>(null);
  // Action history the heatmap is conditioned on. Empty string = start of
  // the flop, OOP to act. The Solver page exposes this as a small selector
  // so users can flip between e.g. "" and "check" or "bet0".
  const [history, setHistory] = useState<string>("");

  useEffect(() => {
    return () => {
      eventSourceRef.current?.close();
    };
  }, []);

  const onSubmit = useCallback(async (config: SolverConfigPayload) => {
    eventSourceRef.current?.close();
    try {
      const jobId = await startSolve(config);
      dispatch({ type: "start", jobId, config });
      const es = streamProgress(jobId);
      eventSourceRef.current = es;
      es.onmessage = (msg) => {
        if (!msg.data) return;
        try {
          const evt = JSON.parse(msg.data) as ProgressEvent;
          dispatch({ type: "progress", evt });
          if (evt.done) {
            dispatch({ type: "done" });
            es.close();
          }
        } catch (err) {
          console.error("bad SSE payload", err);
        }
      };
      es.onerror = () => {
        if (state.status === "running") {
          dispatch({ type: "error", message: "stream closed unexpectedly" });
        }
        es.close();
      };
    } catch (err) {
      const message = err instanceof Error ? err.message : "solve failed";
      dispatch({ type: "error", message });
    }
  }, [state.status]);

  const series = [
    {
      id: "exploit",
      color: "#8b5cf6",
      data: state.points,
    },
  ];

  return (
    <div className="grid grid-cols-12 gap-4">
      <aside className="col-span-3">
        <SolverConfig onSubmit={onSubmit} running={state.status === "running"} />
        {state.error && (
          <div className="mt-3 rounded-md border border-red-500/40 bg-red-500/10 p-3 text-xs text-red-300">
            {state.error}
          </div>
        )}
        {state.jobId && (
          <div className="mt-3 rounded-md border border-line bg-panel p-3 text-xs text-zinc-400">
            <div>
              <span className="text-zinc-500">job: </span>
              <span className="font-mono">{state.jobId}</span>
            </div>
            <div>
              <span className="text-zinc-500">status: </span>
              <span>{state.status}</span>
            </div>
            {state.points.length > 0 && (
              <div>
                <span className="text-zinc-500">expl: </span>
                <span>{state.points[state.points.length - 1].exploitability.toFixed(4)}</span>
              </div>
            )}
          </div>
        )}
      </aside>
      <section className="col-span-9 space-y-4">
        <ConvergenceChart series={series} />
        <RangePanel
          jobId={state.jobId}
          status={state.status}
          history={history}
          onHistoryChange={setHistory}
        />
      </section>
    </div>
  );
}

function RangePanel({
  jobId,
  status,
  history,
  onHistoryChange,
}: {
  jobId: string | null;
  status: Status;
  history: string;
  onHistoryChange: (h: string) => void;
}) {
  const [hovered, setHovered] = useState<string | null>(null);
  const [hoverInfo, setHoverInfo] = useState<{
    hole: [string, string];
    bucket: number;
    infosetKey: number;
  } | null>(null);
  const [hoveredStrategy, setHoveredStrategy] = useState<number[] | null>(null);
  const [hoverError, setHoverError] = useState<string | null>(null);
  // (label, history) → CellStrategy. History is part of the key so flipping
  // the selector doesn't show stale colors.
  const cellCache = useRef(new Map<string, CellStrategy>());

  useEffect(() => {
    cellCache.current.clear();
  }, [jobId, history]);

  const resolve = useCallback(
    (label: string): CellStrategy | null => {
      const key = `${history}::${label}`;
      return cellCache.current.get(key) ?? null;
    },
    [history],
  );

  const onHover = useCallback(
    async (label: string) => {
      setHovered(label);
      setHoverError(null);
      if (!jobId || status !== "done") {
        setHoveredStrategy(null);
        setHoverInfo(null);
        return;
      }
      try {
        const res = await fetchStrategyForHand(jobId, label, history);
        setHoveredStrategy(res.probabilities);
        setHoverInfo({
          hole: res.hole,
          bucket: res.bucket,
          infosetKey: res.infoset_key,
        });
        // Materialize a (bet, check, fold) breakdown for the cell color.
        // Action ordering from the engine depends on history:
        //   facing nothing: [check, bet0, bet1, ..., allin]
        //   facing a bet:   [fold, call, raise0, ..., allin]
        const probs = res.probabilities;
        const facingBet = history.endsWith("bet0") || history.endsWith("bet1") ||
                          history.endsWith("betA") || history.endsWith("raise0") ||
                          history.endsWith("raise1") || history.endsWith("raiseA");
        const cell: CellStrategy = facingBet
          ? {
              fold: probs[0] ?? 0,
              check: probs[1] ?? 0, // "call" colored as the passive (check/call) action
              bet: probs.slice(2).reduce((a, b) => a + b, 0),
            }
          : {
              fold: 0,
              check: probs[0] ?? 0,
              bet: probs.slice(1).reduce((a, b) => a + b, 0),
            };
        cellCache.current.set(`${history}::${label}`, cell);
      } catch (err) {
        setHoveredStrategy(null);
        setHoverInfo(null);
        const message = err instanceof Error ? err.message : "lookup failed";
        setHoverError(message);
      }
    },
    [jobId, status, history],
  );

  return (
    <div className="grid grid-cols-12 gap-4">
      <div className="col-span-8 space-y-3">
        <RangeHeatmap resolve={resolve} onHover={onHover} hovered={hovered} />
        <div className="rounded-lg border border-line bg-panel p-3">
          <label className="flex items-center gap-3 text-xs">
            <span className="uppercase tracking-wide text-zinc-500">History</span>
            <input
              type="text"
              value={history}
              onChange={(e) => onHistoryChange(e.target.value)}
              placeholder="(empty) or e.g. check, check.bet0"
              className="flex-1 rounded-md border border-line bg-ink px-2 py-1 font-mono text-xs text-zinc-200 focus:border-accent-purple focus:outline-none"
            />
            <span className="text-zinc-500">
              actor: {history.split(".").filter(Boolean).length % 2 === 0 ? "OOP" : "IP"}
            </span>
          </label>
        </div>
      </div>
      <div className="col-span-4 space-y-3">
        {hoveredStrategy ? (
          <ActionDistribution
            title={hovered ?? "Action mix"}
            actions={hoveredStrategy.map((_, i) => `a${i}`)}
            probabilities={hoveredStrategy}
          />
        ) : (
          <div className="rounded-lg border border-line bg-panel p-4 text-xs text-zinc-500">
            {status === "done"
              ? "Hover a combo to inspect its action mix."
              : "Run a solve to populate the range."}
          </div>
        )}
        {hoverError && (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 font-mono text-[10px] text-amber-300">
            {hoverError}
          </div>
        )}
        {hoverInfo && (
          <div className="rounded-md border border-line bg-panel px-3 py-2 font-mono text-[10px] text-zinc-500">
            <div>
              hole {hoverInfo.hole[0]}
              {hoverInfo.hole[1]}
            </div>
            <div>bucket {hoverInfo.bucket}</div>
            <div>key {hoverInfo.infosetKey}</div>
          </div>
        )}
      </div>
    </div>
  );
}
