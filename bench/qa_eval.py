#!/usr/bin/env python3
import json
import os
import re
import sys
from pathlib import Path
import subprocess

ENDPOINT = os.environ.get("LMSTUDIO_ENDPOINT", "http://127.0.0.1:1234/v1")
CHAT_MODEL = os.environ.get("LMSTUDIO_CHAT_MODEL")

RESULTS_DIR = Path(os.environ.get("OUT_DIR", "bench/results"))
QUERIES = json.load(open("bench/queries.json"))


def read_file_lines(path):
    try:
        return Path(path).read_text().splitlines()
    except Exception:
        return []


def snippet_from_lines(lines, start, end, pad=2):
    s = max(1, start - pad)
    e = min(len(lines), end + pad)
    chunk = lines[s-1:e]
    return s, e, "\n".join(chunk)


def load_coderev_context(query_id, top_k=5):
    data = json.load(open(RESULTS_DIR / f"coderev_{query_id}.json"))
    results = data.get("data", {}).get("results", [])[:top_k]
    ctx = []
    for r in results:
        path = r["path"]
        ls = r["line_start"]
        le = r["line_end"]
        lines = read_file_lines(path)
        s, e, text = snippet_from_lines(lines, ls, le)
        ctx.append({"path": path, "line_start": s, "line_end": e, "text": text})
    return ctx


def load_coderev_context(query_id, top_k=5):
    data = json.load(open(RESULTS_DIR / f"coderev_{query_id}.json"))
    if isinstance(data, list):
        results = data[:top_k]
    else:
        results = data.get("results", [])[:top_k]
    ctx = []
    for r in results:
        ctx.append({
            "path": r["file_path"],
            "line_start": r["start_line"],
            "line_end": r["end_line"],
            "text": r["content"],
        })
    return ctx


def load_rg_context(query_id, top_k=5):
    txt = Path(RESULTS_DIR / f"rg_{query_id}.txt").read_text().splitlines()
    ctx = []
    for line in txt[:top_k]:
        m = re.match(r"^(.*?):(\d+):", line)
        if not m:
            continue
        path = m.group(1)
        line_no = int(m.group(2))
        lines = read_file_lines(path)
        s, e, text = snippet_from_lines(lines, line_no, line_no)
        ctx.append({"path": path, "line_start": s, "line_end": e, "text": text})
    return ctx


def load_rag_context(query_id, top_k=5):
    data = json.load(open(RESULTS_DIR / f"rag_{query_id}.json"))
    results = data.get("results", [])[:top_k]
    ctx = []
    for r in results:
        ctx.append({
            "path": r["path"],
            "line_start": r["line_start"],
            "line_end": r["line_end"],
            "text": r["text"],
        })
    return ctx


def call_chat(prompt):
    if not CHAT_MODEL:
        raise RuntimeError("LMSTUDIO_CHAT_MODEL is not set")
    url = f"{ENDPOINT}/chat/completions"
    payload = json.dumps({
        "model": CHAT_MODEL,
        "messages": [
            {"role": "system", "content": "Answer using only the provided context. If the context is insufficient, say so explicitly."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 256,
    })
    cmd = ["curl", "-s", "-X", "POST", url, "-H", "Content-Type: application/json", "-d", payload]
    raw = subprocess.check_output(cmd, text=True)
    data = json.loads(raw)
    if "choices" not in data:
        raise RuntimeError(f"LMStudio response missing choices: {data}")
    return data["choices"][0]["message"]["content"]


def trim_text(text, max_chars=1500):
    if len(text) <= max_chars:
        return text
    return text[:max_chars] + "\n...[truncated]..."


def format_context(ctx):
    parts = []
    for i, c in enumerate(ctx, 1):
        parts.append(
            f"[C{i}] {c['path']}:{c['line_start']}-{c['line_end']}\n{trim_text(c['text'])}"
        )
    return "\n\n".join(parts)


def run():
    tools = {
        "coderev": load_coderev_context,
        "coderev": load_coderev_context,
        "rg": load_rg_context,
        "rag": load_rag_context,
    }

    for q in QUERIES:
        for tool, loader in tools.items():
            ctx = loader(q["id"])
            prompt = f"Question: {q['question']}\n\nContext:\n{format_context(ctx)}\n\nAnswer with short citations like [C1], [C2]."
            answer = call_chat(prompt)
            out_path = RESULTS_DIR / f"qa_{tool}_{q['id']}.json"
            out_path.write_text(json.dumps({
                "tool": tool,
                "query_id": q["id"],
                "question": q["question"],
                "answer": answer,
                "context": ctx,
            }, indent=2))


if __name__ == "__main__":
    try:
        run()
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
