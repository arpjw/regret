"use client";

import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

export interface Series {
  id: string;
  color: string;
  data: { iteration: number; exploitability: number }[];
}

interface Props {
  series: Series[];
  height?: number;
}

export function ConvergenceChart({ series, height = 280 }: Props) {
  const combined = mergeSeries(series);
  return (
    <div className="rounded-lg border border-line bg-panel p-4">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-zinc-400">
          Exploitability
        </h2>
        <div className="text-xs text-zinc-500">
          {series.reduce((acc, s) => acc + s.data.length, 0)} samples
        </div>
      </div>
      <div style={{ width: "100%", height }}>
        <ResponsiveContainer>
          <LineChart data={combined} margin={{ top: 8, right: 16, bottom: 0, left: 0 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="#1f2330" />
            <XAxis
              dataKey="iteration"
              stroke="#6b7280"
              fontSize={11}
              type="number"
              domain={["dataMin", "dataMax"]}
            />
            <YAxis
              stroke="#6b7280"
              fontSize={11}
              scale="log"
              domain={["auto", "auto"]}
              tickFormatter={(v) => v.toFixed(2)}
              allowDataOverflow
            />
            <Tooltip
              contentStyle={{ background: "#11141c", border: "1px solid #1f2330" }}
              labelStyle={{ color: "#a1a1aa" }}
            />
            <Legend />
            {series.map((s) => (
              <Line
                key={s.id}
                dataKey={s.id}
                stroke={s.color}
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
                type="monotone"
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

function mergeSeries(series: Series[]) {
  // Build a single dataset keyed by iteration so all lines share an x-axis.
  const xs = new Set<number>();
  for (const s of series) for (const pt of s.data) xs.add(pt.iteration);
  const sorted = [...xs].sort((a, b) => a - b);
  return sorted.map((it) => {
    const row: Record<string, number> = { iteration: it };
    for (const s of series) {
      const pt = s.data.find((p) => p.iteration === it);
      if (pt) row[s.id] = Math.max(pt.exploitability, 1e-4);
    }
    return row;
  });
}
