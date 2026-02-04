#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-bench/results}"
mkdir -p "$OUT_DIR"

python3 - <<'PY'
import json, subprocess, os

queries = json.load(open('bench/queries.json'))
out_dir = os.environ.get('OUT_DIR', 'bench/results')

for q in queries:
    out = os.path.join(out_dir, f"rg_{q['id']}.txt")
    time_out = os.path.join(out_dir, f"rg_{q['id']}.time")
    cmd = ['rg','-n',q['query'],'-S','.']
    with open(out, 'w') as f_out, open(time_out, 'w') as f_time:
        subprocess.run(['/usr/bin/time','-p',*cmd], stdout=f_out, stderr=f_time, check=False)
PY
