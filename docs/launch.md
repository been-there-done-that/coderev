# Coderev Launch — One‑Pager

## What is Coderev?
Coderev is a **graph‑grounded code intelligence engine**. It builds a verified semantic graph of symbols and relationships, then layers semantic search on top — giving LLMs **precise, structural context** instead of noisy chunks.

## Why it matters
Most AI code search tools treat code as text. Coderev treats it like a compiler:
- **Verified callers/callees**
- **Impact analysis**
- **Symbol‑level retrieval**
- **Local‑first, inspectable SQLite substrate**

## Who it’s for
- Engineers working on large repos
- Teams doing refactors or dependency analysis
- AI agents that need grounded context

## Quick pitch
> “Coderev is compiler‑grade retrieval for AI. Search is semantic, but grounded in a real symbol graph.”

## Try it
```bash
coderev index --path /path/to/your/project
coderev search --query "how does auth work?"
coderev callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

## Benchmarks
We ship a reproducible benchmark suite that compares Coderev vs coderev vs ripgrep vs RAG.
See `BENCHMARK.md` and `bench/REPORT.md`.

## Ask
If you try Coderev, open an issue with:
- what worked
- what failed
- what you wish it did

That feedback directly shapes roadmap.
