#!/bin/bash
# Run after all four per-experiment JSONs exist: merges, plots, then
# rewrites the paper sections with real numbers.
set -euo pipefail
cd "$(dirname "$0")"

python3 merge.py
python3 make_figures.py
python3 update_paper.py
echo "ok"
