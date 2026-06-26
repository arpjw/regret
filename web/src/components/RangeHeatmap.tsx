"use client";

import { useMemo, useState } from "react";
import { buildMatrix, RANKS, type HandCell } from "@/lib/hands";

export interface CellStrategy {
  bet: number;
  check: number;
  fold: number;
}

interface Props {
  /** Lookup that resolves a hand cell to its action mix; `null` while loading. */
  resolve: (label: string) => CellStrategy | null;
  /** Called when the user hovers a cell. The parent owns the strategy fetch. */
  onHover?: (label: string) => void;
  hovered?: string | null;
}

export function RangeHeatmap({ resolve, onHover, hovered }: Props) {
  const matrix = useMemo(() => buildMatrix(), []);
  const [internalHover, setInternalHover] = useState<string | null>(null);
  const activeHover = hovered ?? internalHover;

  return (
    <div className="rounded-lg border border-line bg-panel p-4">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-zinc-400">Range</h2>
        <div className="text-xs text-zinc-500">{activeHover ?? "hover a combo"}</div>
      </div>
      <div className="grid grid-cols-13 gap-[2px]">
        {matrix.flat().map((cell) => (
          <Cell
            key={`${cell.row}-${cell.col}`}
            cell={cell}
            strategy={resolve(cell.label)}
            active={activeHover === cell.label}
            onHover={() => {
              setInternalHover(cell.label);
              onHover?.(cell.label);
            }}
          />
        ))}
      </div>
      <Legend />
      <style jsx>{`
        .grid-cols-13 {
          grid-template-columns: repeat(13, minmax(0, 1fr));
        }
      `}</style>
    </div>
  );
}

function Cell({
  cell,
  strategy,
  active,
  onHover,
}: {
  cell: HandCell;
  strategy: CellStrategy | null;
  active: boolean;
  onHover: () => void;
}) {
  const bg = colorFor(strategy);
  return (
    <button
      type="button"
      onMouseEnter={onHover}
      className={`aspect-square select-none rounded-[2px] text-[9px] font-medium text-white/90 transition-all ${
        active ? "ring-2 ring-accent-purple" : ""
      }`}
      style={{ background: bg }}
    >
      <span className="block text-center leading-none">{cell.label}</span>
    </button>
  );
}

function Legend() {
  return (
    <div className="mt-3 flex items-center gap-3 text-[10px] uppercase tracking-wider text-zinc-500">
      <Swatch color="#8b5cf6" label="bet" />
      <Swatch color="#14b8a6" label="check" />
      <Swatch color="#6b7280" label="fold" />
    </div>
  );
}

function Swatch({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1">
      <span className="inline-block h-2 w-2 rounded-sm" style={{ background: color }} />
      <span>{label}</span>
    </span>
  );
}

// Map a (bet, check, fold) tuple to a blended cell color. Purple = bet-heavy,
// teal = check-heavy, gray = mostly folding.
function colorFor(s: CellStrategy | null): string {
  if (!s) return "#1f2330";
  const total = s.bet + s.check + s.fold || 1;
  const b = s.bet / total;
  const c = s.check / total;
  const f = s.fold / total;
  // Linear blend in sRGB space — close enough for a heatmap.
  const cols = [
    [139, 92, 246], // purple — bet
    [20, 184, 166], // teal — check
    [107, 114, 128], // gray — fold
  ];
  const w = [b, c, f];
  const r = Math.round(cols[0][0] * w[0] + cols[1][0] * w[1] + cols[2][0] * w[2]);
  const g = Math.round(cols[0][1] * w[0] + cols[1][1] * w[1] + cols[2][1] * w[2]);
  const bl = Math.round(cols[0][2] * w[0] + cols[1][2] * w[1] + cols[2][2] * w[2]);
  return `rgb(${r},${g},${bl})`;
}

// Re-export the rank ordering for callers building parallel UI.
export { RANKS };
