# Coderev

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/been-there-done-that/coderev/release.yml?branch=main)](https://github.com/been-there-done-that/coderev/actions)
[![Version](https://img.shields.io/github/v/release/been-there-done-that/coderev)](https://github.com/been-there-done-that/coderev/releases)

**Coderev builds a verified semantic code graph so you can search, trace, and reason about code like a compiler would.**

Coderev is a local-first engine that parses your repository, resolves symbol relationships, and stores a queryable graph in SQLite. It adds AI-assisted search on top of real call graphs—so results are grounded in your codebase, not just text similarity.

---

## ⚡ Quick Start (macOS/Linux)

```bash
# 1. Install via Homebrew
brew tap been-there-done-that/coderev && brew install coderev

# 2. Initialize in your project
coderev init

# 3. Index and Search
coderev index
coderev search --query "how is auth handled?"
```

---

## 💻 Installation

### macOS & Linux
- **Homebrew**: `brew tap been-there-done-that/coderev && brew install coderev`
- **Shell Script**: `curl -sSL https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.sh | sh`

### Windows
- **Downloads**: Grab `coderev.exe` from the [Releases](https://github.com/been-there-done-that/coderev/releases).
- **PowerShell**: `iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex`

### From Source (Universal)
```bash
git clone https://github.com/been-there-done-that/coderev.git
cd coderev
cargo install --path .
```

---

## 🚀 Post-Installation: First Steps

### 1) Initialize (`coderev init`)
Run this in your project root to create `coderev.toml`. This tells Coderev where to store its database (default: `.coderev/`) and automatically adds the directory to your `.gitignore`.

### 2) Index (`coderev index`)
Build the graph. Use the `--verbose` flag to see the compiler-grade parsing pipeline in action.
```bash
coderev index --verbose
```

### 3) Verify Health
Check if your index is healthy and see symbol coverage:
```bash
coderev stats
```

---

## 🛠️ Practical Usage & Flags

| Need | Command |
| :--- | :--- |
| **Semantic Search** | `coderev search --query "auth" --limit 5` |
| **Trace Callers** | `coderev callers --uri "codescope://..."` |
| **Trace Impact** | `coderev impact --uri "codescope://..." --depth 3` |
| **JSON Output** | `coderev search --query "auth" --json` |
| **Compact JSON** | `coderev search --query "auth" --compact` |
| **Watch Files** | `coderev watch --background` |

---

## 🤖 For AI Agents & Developers

Coderev is designed to be the "intelligent substrate" for AI agents.

### 🔌 MCP Integration
Connect agents like Claude Desktop or ChatGPT directly to your codebase:
1. Run `coderev agent-setup`.
2. Configure your agent to use `coderev mcp`.
3. See the **[MCP Guide](docs/mcp-guide.md)** for detailed setup.

### 🧠 Agentic Skills
Agents can use the **[SKILLS.md](SKILLS.md)** file to understand how to leverage Coderev tools for self-directed code navigation and refactoring.

### 🧪 Verification for Agents
Agents should run `coderev --help` or `coderev stats --json` to verify the environment is correctly configured before starting work.

---

## 📖 Deep Dives

- 🏗️ **[Architecture](docs/architecture.md)**: How the graph engine works.
- 🛠️ **[CLI Reference](docs/cli-reference.md)**: Exhaustive list of commands and flags.
- 📊 **[Benchmarks](docs/benchmarks.md)**: Latency and QA results.
- 🌍 **[Language Support](README.md#language-support)**: Tiered support for Python, Rust, TS, and more.

---

## 🌍 Language Support

| Tier | Languages | Capability |
| :--- | :--- | :--- |
| **Tier 1** | Python | Full AST resolution, deep symbol graph. |
| **Tier 2** | Rust, JS, TS, Go | Baseline parsing, call graph extraction. |
| **Tier 3** | All others | Semantic search via smart chunking. |

---

## 📄 License

MIT © [Been There Done That](https://github.com/been-there-done-that)
