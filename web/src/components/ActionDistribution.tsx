"use client";

interface Props {
  actions: string[];
  probabilities: number[];
  title?: string;
}

const PALETTE = ["#8b5cf6", "#14b8a6", "#f59e0b", "#ef4444", "#3b82f6", "#a855f7"];

export function ActionDistribution({ actions, probabilities, title }: Props) {
  const labels = actions.length === probabilities.length ? actions : probabilities.map((_, i) => `a${i}`);
  return (
    <div className="rounded-lg border border-line bg-panel p-4">
      {title && (
        <h3 className="mb-2 text-sm font-semibold uppercase tracking-wider text-zinc-400">{title}</h3>
      )}
      <div className="flex h-6 overflow-hidden rounded-md border border-line">
        {probabilities.map((p, i) => (
          <div
            key={i}
            className="flex items-center justify-center text-[10px] font-medium text-white transition-all"
            style={{
              width: `${Math.max(p * 100, 0).toFixed(2)}%`,
              background: PALETTE[i % PALETTE.length],
            }}
            title={`${labels[i]}: ${(p * 100).toFixed(1)}%`}
          >
            {p > 0.08 ? `${(p * 100).toFixed(0)}%` : ""}
          </div>
        ))}
      </div>
      <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-zinc-400">
        {labels.map((a, i) => (
          <li key={i} className="flex items-center gap-1">
            <span
              className="inline-block h-2 w-2 rounded-sm"
              style={{ background: PALETTE[i % PALETTE.length] }}
            />
            <span>{a}</span>
            <span className="text-zinc-500">{(probabilities[i] * 100).toFixed(1)}%</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
