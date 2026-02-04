# AI Agent Integration (MCP)

Coderev implements the **Model Context Protocol (MCP)**, allowing AI agents like Claude or ChatGPT to query your codebase directly with grounded, semantic context.

## 1. Automated Setup

Generate the necessary configuration scaffolding for your agents:

```bash
coderev agent-setup
```

This writes `.coderev/mcp.json` pointing to the Coderev MCP server.

## 2. Using with Claude Desktop

To use Coderev with Claude Desktop, add the following to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "coderev": {
      "command": "coderev",
      "args": ["mcp", "--database", "/absolute/path/to/your/project/.coderev/coderev.db"]
    }
  }
}
```

## 3. Available Tools

Once connected, your agent will have access to the following tools:

- `search_code`: Perform semantic search over symbols.
- `get_callers`: Retrieve the call graph for a specific URI.
- `get_callees`: Retrieve called functions for a specific URI.
- `get_impact`: Analyze the ripple effect of changing a symbol.

## Why MCP?
Unlike standard RAG, which retrieves text chunks, Coderev's MCP implementation provides **graph-grounded context**. The agent understands the actual relationships between your functions and classes, leading to significantly higher accuracy in code reasoning and refactoring tasks.
