# Why Coderev

Coderev is a **graph‑grounded code intelligence engine**. Instead of relying on chunks alone, it builds a verified symbol graph (definitions, calls, references) and layers semantic search on top. This makes LLM‑driven workflows more precise, especially for refactors and dependency questions.

## The Core Idea

Most tools stop at vector search over text. Coderev goes deeper:
- Parse → link → embed
- Store a **symbol graph** in SQLite
- Query with graph awareness (callers, callees, impact)

This yields results that are **structurally correct**, not just semantically similar.

## When Coderev Wins

- **Refactors and impact analysis**: “What breaks if I change this?”
- **Architectural questions**: “Who calls this?” “Where does this flow start?”
- **Large repos**: Where naive chunking returns noisy context.

## Benchmarks (Local, Reproducible)

We ran a benchmark suite comparing Coderev vs coderev vs ripgrep vs RAG on this repo.
Artifacts and scripts live in `bench/`.

**Latency (seconds, lower is faster):**

| Query | Coderev | coderev | rg | RAG |
| :--- | ---: | ---: | ---: | ---: |
| watch-background | 1.93 | 0.41 | 0.02 | 0.14 |
| mcp-server | 1.49 | 0.17 | 0.00 | 0.13 |
| embedding-resolver | 1.42 | 0.18 | 0.00 | 0.14 |
| agent-setup | 1.55 | 0.23 | 0.00 | 0.13 |
| default-db | 1.54 | 0.26 | 0.00 | 0.15 |

**Takeaway:** Coderev is slower than raw text search, but it surfaces **higher‑value, symbol‑level context** for LLMs and engineers.

For full details, see:
- `BENCHMARK.md`
- `bench/REPORT.md`

## Positioning

Coderev is best described as:
- **“Compiler‑grade retrieval for AI.”**
- **“A semantic code graph substrate for agents.”**
- **“Search + graph = fewer hallucinations.”**

If your team needs grounded context, Coderev is worth trying.
