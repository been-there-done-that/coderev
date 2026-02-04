#!/usr/bin/env bash
set -euo pipefail

DB="${Coderev_DB:-.coderev/bench_coderev.db}"
OUT_DIR="${OUT_DIR:-bench/results}"

mkdir -p "$OUT_DIR"

python3 - <<'PY'
import json, subprocess, os, shlex

queries = json.load(open('bench/queries.json'))

db = os.environ.get('Coderev_DB', '.coderev/bench_coderev.db')
out_dir = os.environ.get('OUT_DIR', 'bench/results')

for q in queries:
    out = os.path.join(out_dir, f"coderev_{q['id']}.json")
    time_out = os.path.join(out_dir, f"coderev_{q['id']}.time")
    cmd = [
        'target/release/coderev','search','--query',q['query'],'--database',db,'--json'
    ]
    with open(out, 'w') as f_out, open(time_out, 'w') as f_time:
        subprocess.run(['/usr/bin/time','-p',*cmd], stdout=f_out, stderr=f_time, check=False)
PY
