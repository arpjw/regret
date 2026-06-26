"use client";

import { useCallback, useReducer, useState } from "react";
import { fetchStrategyForHand } from "@/lib/api";
import { ActionDistribution } from "@/components/ActionDistribution";
import { RangeHeatmap, type CellStrategy } from "@/components/RangeHeatmap";

type Street = "flop" | "turn" | "river";

type Texture = "any" | "dry" | "semi-wet" | "wet";

interface ExplorerState {
  street: Street;
  texture: Texture;
  position: "IP" | "OOP";
  jobId: string;
  history: string;
  hovered: string | null;
  hoveredProbs: number[] | null;
  hoverInfo: { hole: [string, string]; bucket: number; key: number } | null;
  hoverError: string | null;
  cache: Record<string, CellStrategy>;
}

type Action =
  | { type: "set"; field: keyof Omit<ExplorerState, "cache" | "hovered" | "hoveredProbs" | "hoverInfo" | "hoverError">; value: string }
  | { type: "hover"; label: string }
  | { type: "strategy"; label: string; probs: number[]; cell: CellStrategy; hole: [string, string]; bucket: number; key: number }
  | { type: "error"; message: string };

const initial: ExplorerState = {
  street: "flop",
  texture: "any",
  position: "OOP",
  jobId: "",
  history: "",
  hovered: null,
  hoveredProbs: null,
  hoverInfo: null,
  hoverError: null,
  cache: {},
};

function reducer(state: ExplorerState, action: Action): ExplorerState {
  switch (action.type) {
    case "set":
      return { ...state, [action.field]: action.value, cache: {} };
    case "hover":
      return { ...state, hovered: action.label, hoverError: null };
    case "strategy":
      return {
        ...state,
        hoveredProbs: action.probs,
        hoverInfo: { hole: action.hole, bucket: action.bucket, key: action.key },
        hoverError: null,
        cache: { ...state.cache, [action.label]: action.cell },
      };
    case "error":
      return { ...state, hoverError: action.message, hoveredProbs: null, hoverInfo: null };
  }
}

export default function ExplorerPage() {
  const [state, dispatch] = useReducer(reducer, initial);
  const [pendingJob, setPendingJob] = useState("");

  const resolve = useCallback(
    (label: string): CellStrategy | null => state.cache[label] ?? null,
    [state.cache],
  );

  const onHover = useCallback(
    async (label: string) => {
      dispatch({ type: "hover", label });
      if (!state.jobId) return;
      try {
        const res = await fetchStrategyForHand(state.jobId, label, state.history);
        const probs = res.probabilities;
        const facingBet = /(?:bet|raise)(?:\d+|A)$/.test(state.history.split(".").pop() ?? "");
        const cell: CellStrategy = facingBet
          ? {
              fold: probs[0] ?? 0,
              check: probs[1] ?? 0,
              bet: probs.slice(2).reduce((a, b) => a + b, 0),
            }
          : {
              fold: 0,
              check: probs[0] ?? 0,
              bet: probs.slice(1).reduce((a, b) => a + b, 0),
            };
        dispatch({
          type: "strategy",
          label,
          probs,
          cell,
          hole: res.hole,
          bucket: res.bucket,
          key: res.infoset_key,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : "lookup failed";
        dispatch({ type: "error", message });
      }
    },
    [state.jobId, state.history],
  );

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-line bg-panel p-4">
        <div className="grid grid-cols-2 gap-3 md:grid-cols-6">
          <Field label="Job ID">
            <input
              type="text"
              value={pendingJob}
              onChange={(e) => setPendingJob(e.target.value)}
              className="explorer-input"
              placeholder="abcd1234"
            />
            <button
              type="button"
              onClick={() =>
                dispatch({ type: "set", field: "jobId", value: pendingJob.trim() })
              }
              className="mt-2 w-full rounded-md bg-accent-purple px-3 py-1.5 text-xs font-semibold text-white hover:bg-violet-500"
            >
              Load
            </button>
          </Field>
          <Field label="History">
            <input
              type="text"
              value={state.history}
              onChange={(e) =>
                dispatch({ type: "set", field: "history", value: e.target.value })
              }
              className="explorer-input"
              placeholder="(empty) or check.bet0"
            />
          </Field>
          <Field label="Street">
            <Select
              value={state.street}
              options={["flop", "turn", "river"]}
              onChange={(v) => dispatch({ type: "set", field: "street", value: v })}
            />
          </Field>
          <Field label="Texture">
            <Select
              value={state.texture}
              options={["any", "dry", "semi-wet", "wet"]}
              onChange={(v) => dispatch({ type: "set", field: "texture", value: v })}
            />
          </Field>
          <Field label="Position">
            <Select
              value={state.position}
              options={["IP", "OOP"]}
              onChange={(v) => dispatch({ type: "set", field: "position", value: v })}
            />
          </Field>
          <Field label="Active job">
            <div className="rounded-md border border-line bg-ink px-3 py-2 font-mono text-xs text-zinc-300">
              {state.jobId || "—"}
            </div>
          </Field>
        </div>
      </div>

      <div className="grid grid-cols-12 gap-4">
        <div className="col-span-8">
          <RangeHeatmap resolve={resolve} onHover={onHover} hovered={state.hovered} />
        </div>
        <div className="col-span-4 space-y-3">
          {state.hoveredProbs ? (
            <ActionDistribution
              title={state.hovered ?? "Action mix"}
              actions={state.hoveredProbs.map((_, i) => `a${i}`)}
              probabilities={state.hoveredProbs}
            />
          ) : (
            <div className="rounded-lg border border-line bg-panel p-4 text-xs text-zinc-500">
              {state.jobId
                ? "Hover a combo to inspect its action mix."
                : "Load a completed job ID to begin."}
            </div>
          )}
          {state.hoverError && (
            <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 font-mono text-[10px] text-amber-300">
              {state.hoverError}
            </div>
          )}
          {state.hoverInfo && (
            <div className="rounded-md border border-line bg-panel px-3 py-2 font-mono text-[10px] text-zinc-500">
              <div>
                hole {state.hoverInfo.hole[0]}
                {state.hoverInfo.hole[1]}
              </div>
              <div>bucket {state.hoverInfo.bucket}</div>
              <div>key {state.hoverInfo.key}</div>
            </div>
          )}
        </div>
      </div>

      <style jsx global>{`
        .explorer-input {
          width: 100%;
          background: #0b0d12;
          border: 1px solid #1f2330;
          color: #e6e8ee;
          border-radius: 6px;
          padding: 8px 10px;
          font-size: 13px;
          outline: none;
        }
        .explorer-input:focus {
          border-color: #8b5cf6;
        }
      `}</style>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="text-xs uppercase tracking-wide text-zinc-500">{label}</span>
      {children}
    </label>
  );
}

function Select({
  value,
  options,
  onChange,
}: {
  value: string;
  options: string[];
  onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="explorer-input"
    >
      {options.map((o) => (
        <option key={o} value={o}>
          {o}
        </option>
      ))}
    </select>
  );
}
