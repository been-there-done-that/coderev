# Coderev: Universal Code Intelligence Substrate

> **"A semantic code graph with AI search"**

Coderev is a semantic code graph engine designed for deep code understanding. Unlike standard text-search tools, Coderev builds a verified graph of symbols and relationships, enabling precise navigation, impact analysis, and AI-powered search that actually understands your architecture.

---

## 🚀 Quick Start Guide

Coderev is designed to behave like a compiler. You give it your source code, and it produces a persistent intelligence layer.

### 1. Index your Code
This is the unified command that parses your code, builds the call graph, and generates semantic embeddings.
```bash
cargo run -- index --path /path/to/your/project --database coderev.db
```
**What happens under the hood:**
- **Phase 1 (Extract)**: Parses source files to identify symbols (functions, classes, variables).
- **Phase 2 (Link)**: Resolves references to build a verified call graph.
- **Phase 3 (Embed)**: Generates mathematical vectors (embeddings) for AI search.

### 2. Search and Explore
Once indexed, you can query your code conceptually or structurally.

#### Semantic (AI) Search
Ask questions about your code's purpose.
```bash
cargo run -- search --query "how does auth work?" --vector
```

#### Graph Analysis
Find actual callers of a function (verified by the graph, not just grep).
```bash
cargo run -- trace callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

---

## 🤖 AI Agent Integration (MCP)

Coderev implements the **Model Context Protocol (MCP)**, allowing AI agents (like Claude Desktop or Cursor) to directly query your codebase.

### Run the MCP Server
```bash
cargo run -- mcp
```
Configure your agent to use this command as an MCP server. The agent will gain access to:
- `search_code`: Find relevant code by concept.
- `get_callers` / `get_callees`: Navigate the call graph.
- `get_impact`: Analyze change impact.

---

## ⚡️ Real-time Indexing

Keep your index fresh automatically with the watcher daemon.

```bash
cargo run -- watch --path /path/to/your/project
```
This listens for file changes and incrementally updates the graph, so your AI always has the latest context.

---

## 🔍 Competitive Depth: Coderev vs. Coderev

Coderev is what code search looks like when built by compiler engineers.

| Feature | Coderev / Vector Search | Coderev |
| :--- | :--- | :--- |
| **Foundation** | Chunks of text | Semantic Code Graph |
| **Search** | Vector guessing | Vector over verified symbols |
| **Logic** | None | Real call graphs & reachability |
| **Reliability** | "Hallucinates" relationships | Verified symbol relationships |
| **AI Protocol** | Custom | **Standard MCP** |
| **Updates** | Manual | **Live Watcher** |

---

## 🛠️ Technical Details

### Language Support
Coderev provides varying levels of depth depending on the language:

- **Deep AST Support**: `Python` (Full symbol extraction and scope resolution).
- **Planned Depth**: `JavaScript`, `TypeScript`, `Rust`, `Go`.
- **Universal Fallback**: All other files (SQL, YAML, Markdown, Terraform, etc.) are indexed using a **semantic fallback chunker**. You never lose coverage, but supported languages get "compiler-grade" precision.

### Why "Substrate"?
Coderev isn't just a search tool; it's a foundation for other tools. Because it exports a clean SQLite-based graph, you can build custom linters, auto-refactorers, or documentation generators on top of it.

---

## 📊 Useful Commands

| Command | Purpose |
| :--- | :--- |
| `index` | **Primary Entry**: Full pipeline (parse → link → embed). |
| `watch` | **Live Mode**: Incrementally update index on file changes. |
| `search` | Find code via keywords or semantic meaning. |
| `trace` | Analyze `callers` or `callees` of a symbol. |
| `impact` | Analyze dependencies (blast radius) of a change. |
| `mcp` | Start Standard MCP server for AI agents. |
| `serve` | Start HTTP server for the UI. |
| `stats` | Show graph density and language distribution. |

---

> *"Coderev is what Coderev would look like if it were built by compiler engineers."*
