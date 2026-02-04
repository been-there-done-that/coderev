#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-bench/results}"
mkdir -p "$OUT_DIR"

python3 bench/qa_eval.py
