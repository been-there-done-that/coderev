# CLI Reference

Coderev provides a comprehensive suite of tools for code intelligence.

## Core Commands

### `index`
Builds the semantic graph and generates embeddings.
- `--path <PATH>`: Directory to index.
- `--database <DB_PATH>`: Target database file.
- `--json`: Output results in JSON format.

### `search`
Performs semantic or exact search.
- `--query <TEXT>`: The search query.
- `--limit <N>`: Maximum number of results.

### `callers` / `callees`
Traverses the verified call graph.
- `--uri <URI>`: The unique identifier of the symbol.

### `impact`
Analyzes the potential impact of changing a specific symbol.
- `--uri <URI>`: The unique identifier of the symbol.
- `--depth <N>`: Traversal depth (default: 3).

### `watch`
Runs a daemon to keep the index fresh.
- `--background`: Run in the background.
- `--status`: Show status of the running daemon.
- `--stop`: Terminate the daemon.

## Infrastructure

### `serve`
Starts the local HTTP server and UI.
- `--port <PORT>`: Default is 3000.

### `mcp`
Starts the Model Context Protocol server over stdio.

### `agent-setup`
Generates MCP configuration for AI agents.

## Global Options
- `-v, --verbose`: Enable detailed logging.
- `--json`: Emit machine-readable JSON.
- `--compact`: Use shorter keys in JSON output.
- `--toon`: Like compact, but with a bit more flavor.
