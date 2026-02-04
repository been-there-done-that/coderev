# Getting Started with Coderev

Coderev is a powerful code intelligence engine designed to help you navigate, search, and understand complex codebases using semantic graphs.

## 1. Installation

### Homebrew (macOS)
```bash
brew tap been-there-done-that/coderev
brew install coderev
```

### From Source
```bash
git clone https://github.com/been-there-done-that/coderev.git
cd coderev
cargo install --path .
```

## 2. Initializing your Project

Navigate to your project root and initialize Coderev:

```bash
coderev init
```

This creates a `coderev.toml` file with default settings and adds `.coderev/` to your `.gitignore`.

## 3. Indexing

The first indexing pass builds the semantic graph and generates embeddings for your symbols.

```bash
coderev index
```

> [!TIP]
> Use the `--verbose` flag to see detailed progress of the indexing pipeline.

## 4. Basic Usage

### Semantic Search
Find code based on meaning, not just keywords:
```bash
coderev search --query "how is authentication handled?"
```

### Tracing Callers
Find out what calls a specific function:
```bash
coderev callers --uri "codescope://my-repo/src/auth.rs#callable:validate_login@10"
```

## Next Steps
- Check out the [CLI Reference](cli-reference.md) for advanced commands.
- See the [MCP Guide](mcp-guide.md) for integrating with AI agents.
