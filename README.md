# Coderev

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/been-there-done-that/coderev/release.yml?branch=main)](https://github.com/been-there-done-that/coderev/actions)
[![Version](https://img.shields.io/github/v/release/been-there-done-that/coderev)](https://github.com/been-there-done-that/coderev/releases)

**Coderev builds a verified semantic code graph so you can search, trace, and reason about code like a compiler would.**

Coderev is a local-first engine that parses your repository, resolves symbol relationships, and stores a queryable graph in SQLite. It adds AI-assisted search on top of real call graphs—so results are grounded in your codebase, not just text similarity.

---

## ⚡ Quick Start

> [!TIP]
> Use these one-liners to get started immediately.

### macOS & Linux
```bash
# Install
brew tap been-there-done-that/coderev && brew install coderev

# Initialize & Index
coderev init && coderev index

# Search
coderev search --query "how is auth handled?"
```

### Windows (PowerShell)
```powershell
# Install
iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex

# Initialize & Index
coderev init; coderev index

# Search
coderev search --query "how is auth handled?"
```

---

## 💻 Installation

### macOS & Linux (Unix)

> [!NOTE]
> Homebrew is the recommended method for macOS users.

**Homebrew:**
```bash
brew tap been-there-done-that/coderev
brew install coderev
```

**Direct Script:**
```bash
curl -sSL https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.sh | sh
```

### Windows

**PowerShell (Automatic):**
```powershell
iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex
```

**Manual Binary:**
1. Download `coderev.exe` from the [Releases](https://github.com/been-there-done-that/coderev/releases).
2. Add the downloaded directory to your system `PATH`.

### From Source (Universal)

```bash
git clone https://github.com/been-there-done-that/coderev.git
cd coderev
cargo install --path .
```

---

## 🚀 Post-Installation: Setup & Verification

### 1. Initialize (`coderev init`)
Run this in your project root to create `coderev.toml`. This tells Coderev where to store its database and ensures `.coderev/` is added to your `.gitignore`.
```bash
coderev init
```

### 2. Build the Graph (`coderev index`)
Build the semantic graph and generate embeddings. 
```bash
# Standard index
coderev index

# Verbose mode (see the compiler pipeline)
coderev index --verbose
```

### 3. Verify Health
Ensure your installation is correct and see your codebase coverage:
```bash
# Check stats
coderev stats

# Verify CLI version
coderev --version
```

---

## 🛠️ Detailed Usage & Flags

| Goal | Command |
| :--- | :--- |
| **Semantic Search** | `coderev search --query "auth" --limit 5` |
| **Trace Callers** | `coderev callers --uri "codescope://..."` |
| **Impact Analysis** | `coderev impact --uri "codescope://..." --depth 3` |
| **Machine Output** | `coderev search --query "auth" --json` |
| **Compact Output** | `coderev search --query "auth" --compact` |
| **Background Sync** | `coderev watch --background` |

---

## 🤖 For AI Agents

Coderev is the ultimate "knowledge substrate" for AI agents.

### 🔌 MCP Integration (Claude/ChatGPT)
Connect agents directly to your codebase using the **Model Context Protocol**:
```bash
# Setup MCP config
coderev agent-setup

# Start the server (usually called by the agent)
coderev mcp --database .coderev/coderev.db
```
> [!IMPORTANT]
> See the **[MCP Guide](docs/mcp-guide.md)** for full agent configuration.

### 🧠 Agentic Skills
AI agents should refer to **[SKILLS.md](SKILLS.md)** to learn how to expertly navigate and refactor code using Coderev's graph-grounded tools.

---

## 📖 Deep Dives

- 🏗️ **[Architecture](docs/architecture.md)**: How the graph engine works.
- 🛠️ **[CLI Reference](docs/cli-reference.md)**: Full list of commands.
- 📊 **[Benchmarks](docs/benchmarks.md)**: Proof of performance.
- 🌍 **[Language Support](docs/language-support.md)**: Tiered support details.

---

## 📄 License

MIT © [Been There Done That](https://github.com/been-there-done-that) ([LICENSE](LICENSE))
