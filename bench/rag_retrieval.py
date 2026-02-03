#!/usr/bin/env python3
import argparse
import json
import os
import subprocess
import sys
from math import sqrt
from pathlib import Path
import subprocess

ENDPOINT = os.environ.get("LMSTUDIO_ENDPOINT", "http://127.0.0.1:1234/v1")
EMBED_MODEL = os.environ.get("LMSTUDIO_EMBED_MODEL", "text-embedding-nomic-embed-text-v1.5")

CHUNK_SIZE = int(os.environ.get("RAG_CHUNK_SIZE", "2000"))
CHUNK_OVERLAP = int(os.environ.get("RAG_CHUNK_OVERLAP", "200"))
MAX_BYTES = int(os.environ.get("RAG_MAX_BYTES", "200000"))

INDEX_PATH = Path("bench/rag_index.jsonl")


def get_files():
    out = subprocess.check_output(["git", "ls-files"], text=True)
    files = [line.strip() for line in out.splitlines() if line.strip()]
    return files


def read_text(path: Path):
    if not path.is_file():
        return None
    data = path.read_bytes()
    if len(data) > MAX_BYTES:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def chunk_text(text: str):
    chunks = []
    start = 0
    while start < len(text):
        end = min(len(text), start + CHUNK_SIZE)
        chunk = text[start:end]
        chunks.append((start, end, chunk))
        if end == len(text):
            break
        start = max(0, end - CHUNK_OVERLAP)
    return chunks


def line_range(text: str, start: int, end: int):
    before = text[:start]
    chunk = text[start:end]
    line_start = before.count("\n") + 1
    line_end = line_start + chunk.count("\n")
    return line_start, line_end


def embed_texts(texts):
    url = f"{ENDPOINT}/embeddings"
    payload = json.dumps({"model": EMBED_MODEL, "input": texts})
    tmp_path = Path("bench/.rag_payload.json")
    tmp_path.write_text(payload)
    cmd = [
        "curl",
        "-s",
        "-X",
        "POST",
        url,
        "-H",
        "Content-Type: application/json",
        "--data-binary",
        f"@{tmp_path}",
    ]
    raw = subprocess.check_output(cmd, text=True)
    data = json.loads(raw)
    return [item["embedding"] for item in data["data"]]


def cosine(a, b):
    dot = sum(x*y for x, y in zip(a, b))
    na = sqrt(sum(x*x for x in a))
    nb = sqrt(sum(y*y for y in b))
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)


def build_index():
    files = get_files()
    items = []
    batch_texts = []
    batch_meta = []

    for f in files:
        path = Path(f)
        text = read_text(path)
        if text is None:
            continue
        for start, end, chunk in chunk_text(text):
            if not chunk.strip():
                continue
            ls, le = line_range(text, start, end)
            batch_texts.append(chunk)
            batch_meta.append({
                "path": f,
                "line_start": ls,
                "line_end": le,
                "text": chunk,
            })
            if len(batch_texts) >= 32:
                embeds = embed_texts(batch_texts)
                for meta, emb in zip(batch_meta, embeds):
                    meta["embedding"] = emb
                    items.append(meta)
                batch_texts.clear()
                batch_meta.clear()

    if batch_texts:
        embeds = embed_texts(batch_texts)
        for meta, emb in zip(batch_meta, embeds):
            meta["embedding"] = emb
            items.append(meta)

    with INDEX_PATH.open("w") as f:
        for item in items:
            f.write(json.dumps(item))
            f.write("\n")


def load_index():
    items = []
    with INDEX_PATH.open() as f:
        for line in f:
            items.append(json.loads(line))
    return items


def retrieve(query: str, top_k: int):
    items = load_index()
    q_emb = embed_texts([query])[0]
    scored = []
    for item in items:
        score = cosine(q_emb, item["embedding"])
        scored.append((score, item))
    scored.sort(key=lambda x: x[0], reverse=True)
    return [
        {
            "score": score,
            "path": item["path"],
            "line_start": item["line_start"],
            "line_end": item["line_end"],
            "text": item["text"],
        }
        for score, item in scored[:top_k]
    ]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rebuild", action="store_true")
    parser.add_argument("--top-k", type=int, default=5)
    parser.add_argument("--query", type=str)
    parser.add_argument("--output", type=str)
    args = parser.parse_args()

    if args.rebuild or not INDEX_PATH.exists():
        build_index()

    if args.query:
        results = retrieve(args.query, args.top_k)
        if args.output:
            with open(args.output, "w") as f:
                json.dump({"query": args.query, "results": results}, f, indent=2)
        else:
            print(json.dumps({"query": args.query, "results": results}, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
