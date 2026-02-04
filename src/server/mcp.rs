//! MCP server implementation with direct JSON-RPC handling
//! This bypasses the mcp_sdk_rs server to allow simpler initialization like coderev

use std::sync::Arc;
use crate::storage::SqliteStore;
use crate::query::QueryEngine;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct GraphArgs {
    uri: String,
    depth: Option<u32>,
}

pub struct McpService {
    store: Arc<SqliteStore>,
}

impl McpService {
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self { store }
    }

    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(req) => self.handle_request(req).await,
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                },
            };

            // Only send response if there was an id (not a notification)
            if response.id != Value::Null || response.error.is_some() {
                let json = serde_json::to_string(&response)?;
                stdout.write_all(json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.clone().unwrap_or(Value::Null);
        
        match req.method.as_str() {
            "initialize" => {
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(serde_json::json!({
                        "protocolVersion": "2024-11-05",
                        "serverInfo": {
                            "name": "coderev",
                            "version": "1.0.0"
                        },
                        "capabilities": {
                            "tools": {}
                        }
                    })),
                    error: None,
                }
            }
            "notifications/initialized" | "initialized" => {
                // No response for notifications
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: None,
                }
            }
            "tools/list" => {
                let tools = serde_json::json!({
                    "tools": [
                        {
                            "name": "search_code",
                            "description": "Semantic code search. Find symbols by natural language query.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string", "description": "Natural language search query" },
                                    "limit": { "type": "integer", "description": "Max results (default: 10)" }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "get_callers",
                            "description": "Find all functions that call the specified symbol.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "uri": { "type": "string", "description": "Symbol URI" },
                                    "depth": { "type": "integer", "description": "Traversal depth (default: 1)" }
                                },
                                "required": ["uri"]
                            }
                        },
                        {
                            "name": "get_callees",
                            "description": "Find all functions called by the specified symbol.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "uri": { "type": "string", "description": "Symbol URI" },
                                    "depth": { "type": "integer", "description": "Traversal depth (default: 1)" }
                                },
                                "required": ["uri"]
                            }
                        },
                        {
                            "name": "get_impact",
                            "description": "Analyze impact of changes to a symbol.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "uri": { "type": "string", "description": "Symbol URI" },
                                    "depth": { "type": "integer", "description": "Traversal depth (default: 3)" }
                                },
                                "required": ["uri"]
                            }
                        }
                    ]
                });
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(tools),
                    error: None,
                }
            }
            "tools/call" => {
                self.handle_tool_call(id, req.params).await
            }
            _ => {
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", req.method),
                    }),
                }
            }
        }
    }

    async fn handle_tool_call(&self, id: Value, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => return JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Missing params".to_string(),
                }),
            },
        };

        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

        let result_text = match tool_name {
            "search_code" => {
                let args: SearchArgs = match serde_json::from_value(arguments) {
                    Ok(a) => a,
                    Err(e) => return self.error_response(id, -32602, format!("Invalid arguments: {}", e)),
                };
                
                // Use semantic search via embeddings
                match crate::query::EmbeddingEngine::new() {
                    Ok(embedding_engine) => {
                        match embedding_engine.embed_query(&args.query) {
                            Ok(query_vector) => {
                                let engine = QueryEngine::new(&self.store);
                                match engine.search_by_vector(&query_vector, args.limit.unwrap_or(10) as usize) {
                                    Ok(results) => {
                                        let mut output = String::new();
                                        for res in results {
                                            output.push_str(&format!("- {} ({}) [score: {:.2}]\n  {}\n", 
                                                res.symbol.name, res.symbol.kind, res.score, res.symbol.uri.to_uri_string()));
                                            if let Some(sig) = &res.symbol.signature {
                                                output.push_str(&format!("  Sig: {}\n", sig));
                                            }
                                        }
                                        if output.is_empty() {
                                            "No results found.".to_string()
                                        } else {
                                            output
                                        }
                                    }
                                    Err(e) => format!("Search error: {}", e),
                                }
                            }
                            Err(e) => format!("Embedding error: {}", e),
                        }
                    }
                    Err(e) => format!("Failed to load embedding engine: {}", e),
                }
            }
            "get_callers" => {
                let args: GraphArgs = match serde_json::from_value(arguments) {
                    Ok(a) => a,
                    Err(e) => return self.error_response(id, -32602, format!("Invalid arguments: {}", e)),
                };
                
                let engine = QueryEngine::new(&self.store);
                match crate::uri::SymbolUri::parse(&args.uri) {
                    Ok(uri) => {
                        match engine.find_callers(&uri, args.depth.unwrap_or(1) as usize) {
                            Ok(callers) => {
                                let mut output = String::new();
                                for sym in callers {
                                    output.push_str(&format!("- {} ({})\n  {}\n", sym.name, sym.kind, sym.uri.to_uri_string()));
                                }
                                if output.is_empty() {
                                    "No callers found.".to_string()
                                } else {
                                    output
                                }
                            }
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    Err(e) => format!("Invalid URI: {}", e),
                }
            }
            "get_callees" => {
                let args: GraphArgs = match serde_json::from_value(arguments) {
                    Ok(a) => a,
                    Err(e) => return self.error_response(id, -32602, format!("Invalid arguments: {}", e)),
                };
                
                let engine = QueryEngine::new(&self.store);
                match crate::uri::SymbolUri::parse(&args.uri) {
                    Ok(uri) => {
                        match engine.find_callees(&uri, args.depth.unwrap_or(1) as usize) {
                            Ok(callees) => {
                                let mut output = String::new();
                                for sym in callees {
                                    output.push_str(&format!("- {} ({})\n  {}\n", sym.name, sym.kind, sym.uri.to_uri_string()));
                                }
                                if output.is_empty() {
                                    "No callees found.".to_string()
                                } else {
                                    output
                                }
                            }
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    Err(e) => format!("Invalid URI: {}", e),
                }
            }
            "get_impact" => {
                let args: GraphArgs = match serde_json::from_value(arguments) {
                    Ok(a) => a,
                    Err(e) => return self.error_response(id, -32602, format!("Invalid arguments: {}", e)),
                };
                
                let engine = QueryEngine::new(&self.store);
                match crate::uri::SymbolUri::parse(&args.uri) {
                    Ok(uri) => {
                        match engine.impact_analysis(&uri, args.depth.unwrap_or(3) as usize) {
                            Ok(impact) => {
                                let mut output = String::new();
                                for res in impact {
                                    let prefix = if res.is_direct() { "🔴 [DIRECT]" } else { "🟠 [INDIRECT]" };
                                    output.push_str(&format!("{} {} ({})\n  {}\n", prefix, res.symbol.name, res.symbol.kind, res.symbol.uri.to_uri_string()));
                                }
                                if output.is_empty() {
                                    "No impact found.".to_string()
                                } else {
                                    output
                                }
                            }
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    Err(e) => format!("Invalid URI: {}", e),
                }
            }
            _ => format!("Unknown tool: {}", tool_name),
        };

        JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": result_text
                }]
            })),
            error: None,
        }
    }

    fn error_response(&self, id: Value, code: i32, message: String) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}
