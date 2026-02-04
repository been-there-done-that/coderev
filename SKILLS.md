# Skill: Coderev Code Intelligence

Use Coderev to navigate, search, and reason about codebases using a verified semantic graph. Unlike basic RAG, Coderev provides grounded relationships (callers/callees) and compiler-grade symbol resolution.

## Capabilities
- **Semantic Search**: Find code based on conceptual meaning using vector embeddings.
- **Call Graph Traversal**: Discover exact callers and callees of any function or method.
- **Impact Analysis**: Predict the ripple effect of code changes across the codebase.
- **MCP Integration**: Fully compatible with the Model Context Protocol for seamless agentic workflows.

## Setup & Verification

### 1. Installation
Agents can install Coderev using the following command:
```bash
# macOS/Linux
brew tap been-there-done-that/coderev && brew install coderev

# Source (Universal)
cargo install --path .
```

### 2. Initialization
Prepare the project for intelligence:
```bash
coderev init
coderev index
```

### 3. Verification
Verify the skill is active and healthy:
```bash
coderev stats --json
```

## Agentic Workflow Examples

### Finding a Symbol
"Search for where authentication logic is defined and show me its callers."
1. `coderev search --query "authentication logic" --limit 1`
2. `coderev callers --uri <URI_FROM_SEARCH>`

### Analyzing Impact
"What happens if I change the signature of `validate_session`?"
1. `coderev impact --uri "codescope://repo/src/auth.rs#callable:validate_session" --depth 2`

## Tool Mapping
| Agent Action | CLI Command |
| :--- | :--- |
| Search Meaning | `coderev search --query "<concept>"` |
| Trace Callers | `coderev callers --uri "<uri>"` |
| Trace Callees | `coderev callees --uri "<uri>"` |
| Impact Map | `coderev impact --uri "<uri>"` |
| System Health | `coderev stats` |
