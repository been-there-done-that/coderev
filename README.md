# Coderev — Universal Code Intelligence Substrate

**Coderev builds a verified semantic code graph so you can search, trace, and reason about code like a compiler would.**

Coderev is a local-first engine that parses your repository, resolves symbol relationships, and stores a queryable graph in SQLite. It adds AI-assisted search on top of real call graphs — so results are grounded in your codebase, not just text similarity.

---

## Highlights

- **Compiler-grade graph**: Symbols, definitions, references, callers, callees, and impact paths.
- **Local AI search**: Semantic search over real symbols and their implementation context.
- **MCP-ready**: Connect AI agents via the Model Context Protocol.
- **Live indexing**: Watch mode keeps your graph fresh.
- **SQLite substrate**: Build your own tooling on top of an open, inspectable store.

---

## Why Coderev

Coderev isn’t “just another semantic search.” It’s **graph‑grounded retrieval**:
- **Verified structure**: Callers, callees, and impact paths come from a semantic graph, not guessed from chunks.
- **Better LLM context**: Symbols and relationships map to how engineers reason about code.
- **Local‑first + deterministic**: You can inspect and trust the substrate.

See `WHY_Coderev.md` for a concise positioning and benchmark highlights.
See `docs/launch.md` for a one‑page launch note you can share.

---

## Try Coderev

1. Index your repo:
```bash
coderev index --path /path/to/your/project
```

2. Ask real questions:
```bash
coderev search --query "where is auth handled?"
coderev callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

3. (Optional) Start the MCP server:
```bash
coderev mcp --database .coderev/coderev.db
```

If this saves you time, open an issue and tell us what worked and what didn’t.

---

## What Coderev Supports

### Language Coverage

- **Deep AST support**: `Python`
- **Baseline parsing**: `JavaScript`, `TypeScript`, `Rust`, `Go`
- **Semantic fallback**: All other text-based files are indexed via chunking to keep full repository coverage.

### Interfaces

- **CLI**: Index, search, trace, impact, resolve, stats
- **MCP server**: AI agent integration over stdio
- **HTTP server + UI**: Local UI served by `coderev serve`

---

## Quick Start

### 1) Build release binary

```bash
cargo build --release
```

### 2) Index a repository

```bash
target/release/coderev index --path /path/to/your/project
```

### 2.5) Optional: create a config file

```bash
target/release/coderev init --path /path/to/your/project
```

This writes `coderev.toml` so you can omit `--path` and `--database` later.  
By default the database is stored at `.coderev/coderev.db`, and `.coderev/` is added to `.gitignore`.

## Installation (From Source)

### macOS / Linux

```bash
./install.sh
```

### Windows (PowerShell)

```powershell
./install.ps1
```

### Cargo (local install)

```bash
cargo install --path .
```

Update after changes:

```bash
cargo install --path . --force
```

### Dev vs Release

- **Dev**: `cargo run -- <args>` (fast iteration, slower runtime)
- **Release**: `target/release/coderev <args>` or `cargo install --path .` (optimized, for real usage)

### 3) Search semantically

```bash
target/release/coderev search --query "how does auth work?"
```

### 4) Trace callers / callees

```bash
target/release/coderev callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

---

## How It Works (Illustrated)

### End-to-end pipeline

```mermaid
graph TD
    A[Repo Files] --> B[Tree-sitter Parsers]
    B --> C[Symbol + Scope Extraction]
    C --> D[(SQLite Graph Store)]
    D --> E[Resolver: callers/callees]
    E --> D
    D --> F[FastEmbed Vectors]
    F --> D
    D --> G[Query Engine]
```

### Query flow

```mermaid
sequenceDiagram
    actor Dev
    Dev->>Coderev: search("auth validation")
    Coderev->>SQLite: fetch symbols + vectors
    SQLite-->>Coderev: symbol matches
    Coderev-->>Dev: ranked results w/ URIs
```

---

## CLI Reference (Core)

| Command | Purpose |
| :--- | :--- |
| `index` | Parse → resolve → embed your repository. |
| `init` | Create `coderev.toml` for default paths and database. |
| `search` | Semantic or exact search over symbols. |
| `embed` | Generate embeddings (default model: `all-MiniLM-L6-v2`). |
| `callers` / `callees` | Traverse the verified call graph. |
| `impact` | BFS traversal for change impact analysis. |
| `resolve` | Resolve deferred references after indexing. |
| `stats` | Inspect index and symbol coverage. |
| `watch` | Incremental updates on file change. |
| `serve` | Serve API + UI locally. |
| `mcp` | Start MCP server over stdio. |
| `trace callers|callees` | Alias for callers/callees. |

---

## Structured Output (Automation-Friendly)

Coderev can emit stable, predictable output for scripts and agents:

- `--json` — versioned schema (`schema_version: "1"`).
- `--compact` — same data with shorter keys.
- `--toon` — same as compact, just with a funnier name. We support TOON because why not.

---

## Configuration

Coderev reads `coderev.toml` from the project root when present.

```toml
database = ".coderev/coderev.db"
repo = "my-repo"
path = "."
```

---

## Agent Setup (MCP)

Generate MCP config scaffolding for agents:

```bash
target/release/coderev agent-setup --path /path/to/your/project
```

This writes `.coderev/mcp.json` pointing to `coderev mcp --database .coderev/coderev.db`.

---

## MCP Integration

Coderev implements the **Model Context Protocol (MCP)** so AI agents can query your codebase directly.

```bash
target/release/coderev mcp --database .coderev/coderev.db
```

MCP tools include:
- `search_code`
- `get_callers`
- `get_callees`
- `get_impact`

---

## UI (Local)

Run the local UI + API server:

```bash
target/release/coderev serve --database .coderev/coderev.db
```

UI assets live in `ui/` and are served by the backend when built.

---

## Watcher Daemon

Run in the background:

```bash
target/release/coderev watch --background
```

Check status / stop:

```bash
target/release/coderev watch --status
target/release/coderev watch --stop
```

Daemon state lives in `.coderev/` (`coderev-watch.pid`, `coderev-watch.log`).

---

## Benchmarking

Reproducible benchmark suite (Coderev vs coderev vs rg vs RAG):

```bash
git clean -fdx  # optional: for a clean run
cargo build --release
bench/run_all.sh
python3 bench/report.py
```

Outputs:
- `bench/results/` (raw artifacts)
- `bench/REPORT.md` (summary table)

---

## Design Principles

- **Grounded graph first**: No hallucinated relationships.
- **Local-first**: Everything runs on your machine.
- **Composable substrate**: The SQLite schema is the stable foundation.

---

## Repository Layout

- `src/` — Rust core (indexing, graph, resolver, query)
- `ui/` — Svelte UI
- `queries/` — query logic and experiments
- `test_*` — fixtures and test data

---

## Contributing

Contributions welcome — especially in:
- JS/TS/Rust/Go adapters
- Incremental indexing performance
- Vector index acceleration
- UI polish and usability

---

## License

MIT
