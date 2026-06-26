"use client";

import { useState } from "react";
import type { SolverConfigPayload } from "@/lib/api";

interface Props {
  initial?: Partial<SolverConfigPayload>;
  onSubmit: (config: SolverConfigPayload) => void;
  running: boolean;
}

const DEFAULTS: SolverConfigPayload = {
  matchup: "BTN_vs_BB",
  stack_depth: 100,
  board: ["Ah", "Ks", "7d"],
  bet_sizes: [0.25, 0.5, 0.75],
  card_buckets: 50,
  max_iterations: 1000,
  starting_pot: 6,
  max_raises_per_street: 3,
};

export function SolverConfig({ initial, onSubmit, running }: Props) {
  const merged = { ...DEFAULTS, ...initial };
  const [stackDepth, setStackDepth] = useState<number>(merged.stack_depth);
  const [board, setBoard] = useState<string>(merged.board.join(" "));
  const [betSizes, setBetSizes] = useState<string>(merged.bet_sizes.join(", "));
  const [cardBuckets, setCardBuckets] = useState<number>(merged.card_buckets);
  const [maxIterations, setMaxIterations] = useState<number>(merged.max_iterations);
  const [matchup, setMatchup] = useState<string>(merged.matchup);
  const [startingPot, setStartingPot] = useState<number>(merged.starting_pot);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const sizes = betSizes
      .split(/[,\s]+/)
      .map((s) => Number.parseFloat(s))
      .filter((n) => !Number.isNaN(n) && n > 0);
    const boardCards = board.split(/\s+/).filter((c) => c.length > 0);
    onSubmit({
      matchup,
      stack_depth: stackDepth,
      board: boardCards,
      bet_sizes: sizes,
      card_buckets: cardBuckets,
      max_iterations: maxIterations,
      starting_pot: startingPot,
      max_raises_per_street: merged.max_raises_per_street,
    });
  }

  return (
    <form onSubmit={submit} className="space-y-4 rounded-lg border border-line bg-panel p-4">
      <h2 className="text-sm font-semibold uppercase tracking-wider text-zinc-400">
        Solver configuration
      </h2>
      <Field label="Matchup">
        <select
          value={matchup}
          onChange={(e) => setMatchup(e.target.value)}
          className="input"
        >
          <option>BTN_vs_BB</option>
          <option>SB_vs_BB</option>
          <option>CO_vs_BTN</option>
        </select>
      </Field>
      <Field label="Stack depth (bb)">
        <NumericInput value={stackDepth} onChange={setStackDepth} min={20} max={200} />
      </Field>
      <Field label="Starting pot (bb)">
        <NumericInput value={startingPot} onChange={setStartingPot} min={2} max={200} step={0.5} />
      </Field>
      <Field label="Board (3 cards, e.g. Ah Ks 7d)">
        <input
          type="text"
          value={board}
          onChange={(e) => setBoard(e.target.value)}
          className="input"
          placeholder="Ah Ks 7d"
        />
      </Field>
      <Field label="Bet sizes (pot fractions)">
        <input
          type="text"
          value={betSizes}
          onChange={(e) => setBetSizes(e.target.value)}
          className="input"
          placeholder="0.25, 0.5, 0.75"
        />
      </Field>
      <Field label="Card buckets">
        <NumericInput value={cardBuckets} onChange={setCardBuckets} min={5} max={200} />
      </Field>
      <Field label="Max iterations">
        <NumericInput
          value={maxIterations}
          onChange={setMaxIterations}
          min={50}
          max={20000}
          step={50}
        />
      </Field>
      <button
        type="submit"
        disabled={running}
        className="w-full rounded-md bg-accent-purple px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-violet-500 disabled:cursor-not-allowed disabled:bg-line disabled:text-zinc-500"
      >
        {running ? "Solving…" : "Solve"}
      </button>
      <style jsx>{`
        :global(.input) {
          width: 100%;
          background: #0b0d12;
          border: 1px solid #1f2330;
          color: #e6e8ee;
          border-radius: 6px;
          padding: 8px 10px;
          font-size: 13px;
          outline: none;
        }
        :global(.input:focus) {
          border-color: #8b5cf6;
        }
      `}</style>
    </form>
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

function NumericInput({
  value,
  onChange,
  min,
  max,
  step,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <input
      type="number"
      value={value}
      min={min}
      max={max}
      step={step ?? 1}
      onChange={(e) => onChange(Number.parseFloat(e.target.value))}
      className="input"
    />
  );
}
