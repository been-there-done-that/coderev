#!/usr/bin/env python3
import json
import re
from pathlib import Path

QUERIES = json.load(open('bench/queries.json'))
RESULTS = Path('bench/results')

TOOLS = ['coderev','coderev','rg','rag']


def read_time(tool, qid):
    p = RESULTS / f"{tool}_{qid}.time"
    if not p.exists():
        return None
    m = re.findall(r"real\s+([0-9.]+)", p.read_text())
    return float(m[-1]) if m else None


def qa_summary(tool, qid):
    p = RESULTS / f"qa_{tool}_{qid}.json"
    if not p.exists():
        return None
    data = json.load(open(p))
    answer = data.get('answer','')
    has_citation = bool(re.search(r"\[C\d+\]", answer))
    insufficient = 'insufficient' in answer.lower()
    return {
        'len': len(answer),
        'cit': has_citation,
        'insufficient': insufficient,
    }


def main():
    lines = []
    lines.append("# Coderev vs coderev vs ripgrep vs RAG — Benchmark Report")
    lines.append("")
    lines.append("Generated from `bench/report.py`.")
    lines.append("")

    lines.append("## Latency Summary (seconds)")
    lines.append("")
    header = "| Query | Coderev | coderev | rg | RAG |"
    sep = "| :--- | ---: | ---: | ---: | ---: |"
    lines.extend([header, sep])
    for q in QUERIES:
        t = {tool: read_time(tool, q['id']) for tool in TOOLS}
        def fmt(v):
            return "n/a" if v is None else f"{v:.2f}"
        lines.append(f"| {q['id']} | {fmt(t['coderev'])} | {fmt(t['coderev'])} | {fmt(t['rg'])} | {fmt(t['rag'])} |")

    lines.append("")
    lines.append("## QA Summary (LMStudio)")
    lines.append("")
    lines.append("Legend: `cit` = has citations like `[C1]`, `insuf` = says insufficient context")
    lines.append("")
    header = "| Query | Coderev (cit/insuf) | coderev | rg | RAG |"
    sep = "| :--- | :---: | :---: | :---: | :---: |"
    lines.extend([header, sep])
    for q in QUERIES:
        row = [q['id']]
        for tool in TOOLS:
            s = qa_summary(tool, q['id'])
            if s is None:
                row.append("n/a")
            else:
                row.append(f"{int(s['cit'])}/{int(s['insufficient'])}")
        lines.append(f"| {row[0]} | {row[1]} | {row[2]} | {row[3]} | {row[4]} |")

    Path('bench/REPORT.md').write_text("\n".join(lines) + "\n")


if __name__ == '__main__':
    main()
