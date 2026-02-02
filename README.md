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
cargo run -- callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

---

## 🔍 Competitive Depth: Coderev vs. Coderev

Coderev is what code search looks like when built by compiler engineers.

| Feature | Coderev / Vector Search | Coderev |
| :--- | :--- | :--- |
| **Foundation** | Chunks of text | Semantic Code Graph |
| **Search** | Vector guessing | Vector over verified symbols |
| **Logic** | None | Real call graphs & reachability |
| **Reliability** | "Hallucinates" relationships | Verified symbol relationships |

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
| `search` | Find code via keywords or semantic meaning. |
| `callers` | List all functions that call a specific symbol. |
| `impact` | Analyze the downstream dependencies (blast radius) of a change. |
| `stats` | Show graph density and language distribution. |

---

> *"Coderev is what Coderev would look like if it were built by compiler engineers."*
