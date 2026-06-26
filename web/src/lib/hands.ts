// Canonical 13x13 hand-matrix utilities.

export const RANKS = ["A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2"] as const;
export type Rank = (typeof RANKS)[number];

export type HandKind = "pair" | "suited" | "offsuit";

export interface HandCell {
  label: string;
  row: number;
  col: number;
  kind: HandKind;
}

export function buildMatrix(): HandCell[][] {
  return RANKS.map((r1, row) =>
    RANKS.map((r2, col): HandCell => {
      let kind: HandKind;
      let label: string;
      if (row === col) {
        kind = "pair";
        label = `${r1}${r2}`;
      } else if (col > row) {
        kind = "suited";
        label = `${r1}${r2}s`;
      } else {
        kind = "offsuit";
        label = `${r2}${r1}o`;
      }
      return { label, row, col, kind };
    }),
  );
}

