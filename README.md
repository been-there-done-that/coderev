# Coderev: Universal Code Intelligence Substrate

Coderev is a language-agnostic tool for building a semantic code graph from your codebase. It enables natural language search, deep call graph analysis, and impact assessment for refactoring.

## 🚀 Quick Start Guide

To get meaningful search results, you need to follow these three steps:

### 1. Index your Code
First, Coderev must parse your codebase to identify symbols (functions, classes) and their relationships.
```bash
cargo run -- index --path /path/to/your/project --database coderev.db
```
*This step extracts all code elements and performs an initial cross-reference linking.*

### 2. Generate Semantic Embeddings (CRITICAL for Vector Search)
By default, the database contains text but no "semantic understanding." You must generate embeddings to enable vector search.
```bash
cargo run -- embed --database coderev.db
```
*This uses a local transformer model (`all-MiniLM-L6-v2`) to turn your code into mathematical vectors. This is required to use the `--vector` flag in search.*

### 3. Search and Explore
Now you can perform semantic searches and analysis.

#### Semantic (Natural Language) Search
Find code based on *meaning* rather than just keywords.
```bash
cargo run -- search --query "how to filter activities" --vector --database coderev.db
```

#### Keyword Search
Standard fast text matching across names and documentation.
```bash
cargo run -- search --query "filter" --database coderev.db
```

---

## 🔍 What can I find with Coderev?

Coderev doesn't just find text; it understands the structure of your code.

### 1. "How to..." Queries
With `--vector` search, you can ask conceptual questions:
*   `--query "how do we handle user authentication"`
*   `--query "data validation logic for forms"`

### 2. Trace Callers (Who uses this?)
Instead of grepping for strings, find actual semantic callers.
```bash
# First find the URI of a symbol via search, then:
cargo run -- callers --uri "codescope://my-repo/src/auth.py#callable:validate_login@10"
```

### 3. Impact Analysis (What breaks if I change this?)
See the blast radius of a potential change across multiple levels of the call graph.
```bash
cargo run -- impact --uri "codescope://my-repo/src/db.py#callable:execute_query@50" --depth 3
```

---

## 🛠️ Troubleshooting: "❌ No symbols found"

If your search returns no results, check the following:

1.  **Missing Embeddings**: If you are using the `--vector` flag, you **must** have run `cargo run -- embed` first. If no vectors exist in the database, semantic search will return nothing.
2.  **Wrong Database**: Ensure you are pointing to the correct `.db` file using the `--database` flag.
3.  **Language Support**: Coderev currently has native AST support for **Python, JavaScript/TypeScript, Rust, and Go**. Other file types (YAML, Markdown, etc.) are indexed using a fallback chunker.
4.  **Indexing Depth**: Check if your source files were actually processed during the `index` phase (look for "Processing (AST)" in the logs).

## 📊 Useful Commands

| Command | Purpose |
| :--- | :--- |
| `index` | Scans files and builds the initial symbol graph. |
| `embed` | Generates semantic vectors for AI-powered search. |
| `resolve` | Links references (like function calls) to their definitions. |
| `stats` | Shows how many symbols and relationships are in your database. |
| `search` | Find code via keywords or semantic meaning. |
| `callers` | List all functions that call a specific symbol. |
| `impact` | Analyze the downstream dependencies of a symbol. |
