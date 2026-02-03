#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-bench/results}"
mkdir -p "$OUT_DIR"

python3 bench/rag_retrieval.py --rebuild

python3 - <<'PY'
import json, os, subprocess

queries = json.load(open('bench/queries.json'))
out_dir = os.environ.get('OUT_DIR', 'bench/results')

for q in queries:
    out = os.path.join(out_dir, f"rag_{q['id']}.json")
    time_out = os.path.join(out_dir, f"rag_{q['id']}.time")
    cmd = ['python3','bench/rag_retrieval.py','--query',q['query'],'--top-k','5','--output',out]
    with open(time_out, 'w') as f_time:
        subprocess.run(['/usr/bin/time','-p',*cmd], stdout=subprocess.DEVNULL, stderr=f_time, check=False)
PY
