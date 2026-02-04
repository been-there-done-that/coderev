# Skill: Coderev Code Intelligence

> [!NOTE]
> This skill enables AI agents to navigate and reason about codebases using a verified semantic graph.

## Capability Description
Coderev provides absolute ground truth for symbol relationships (callers/callees) and semantic search. Agents use this to bypass the limitations of standard text-based RAG.

## Quick Installation & Verification

### Unix (macOS/Linux)
```bash
# Install & Setup
brew tap been-there-done-that/coderev && brew install coderev
coderev init && coderev index

# Verify
coderev stats --json
```

### Windows (PowerShell)
```powershell
# Install & Setup
iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex
coderev init; coderev index

# Verify
coderev stats --json
```

---

## Agentic Workflows

### 🔍 Discovery
"Find where the API endpoints are defined."
```bash
coderev search --query "api endpoint definitions" --limit 5
```

### 🔗 Relationship Mapping
"Trace all callers of the `validate` method."
```bash
coderev callers --uri <URI_FROM_SEARCH>
```

### 💥 Impact Analysis
"What breaks if I change the DB schema?"
```bash
coderev impact --uri <SCHEMA_URI> --depth 2
```

---

## Tool Reference for Agents

| Action | Command | Output Mode Suggestion |
| :--- | :--- | :--- |
| **Search** | `coderev search --query "..."` | `--json` |
| **Callers** | `coderev callers --uri "..."` | `--compact` |
| **Callees** | `coderev callees --uri "..."` | `--compact` |
| **Impact** | `coderev impact --uri "..."` | `--json` |
| **Metadata** | `coderev stats` | `--json` |

---

> [!TIP]
> Always use `--json` when pipes are involved for reliable programmatic parsing.
