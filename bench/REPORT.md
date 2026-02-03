# Coderev vs coderev vs ripgrep vs RAG — Benchmark Report

Generated from `bench/report.py`.

## Latency Summary (seconds)

| Query | Coderev | coderev | rg | RAG |
| :--- | ---: | ---: | ---: | ---: |
| watch-background | 1.93 | 0.41 | 0.02 | 0.14 |
| mcp-server | 1.49 | 0.17 | 0.00 | 0.13 |
| embedding-resolver | 1.42 | 0.18 | 0.00 | 0.14 |
| agent-setup | 1.55 | 0.23 | 0.00 | 0.13 |
| default-db | 1.54 | 0.26 | 0.00 | 0.15 |

## QA Summary (LMStudio)

Legend: `cit` = has citations like `[C1]`, `insuf` = says insufficient context

| Query | Coderev (cit/insuf) | coderev | rg | RAG |
| :--- | :---: | :---: | :---: | :---: |
| watch-background | 1/0 | 1/0 | 1/0 | 1/0 |
| mcp-server | 1/0 | 1/0 | 1/0 | 1/0 |
| embedding-resolver | 1/0 | 1/0 | 1/0 | 1/0 |
| agent-setup | 1/0 | 1/0 | 1/0 | 1/0 |
| default-db | 1/0 | 1/0 | 1/0 | 0/0 |
