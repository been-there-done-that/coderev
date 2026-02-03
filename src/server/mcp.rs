use std::sync::Arc;
use crate::storage::SqliteStore;
use crate::query::QueryEngine;
use mcp_sdk_rs::server::{Server, ServerHandler};
use mcp_sdk_rs::types::{
    Tool, ToolResult, ListToolsResult,
    Implementation, ClientCapabilities, ServerCapabilities
};
use mcp_sdk_rs::error::ErrorCode;
use mcp_sdk_rs::transport::stdio::StdioTransport;
use mcp_sdk_rs::error::Error;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::Deserialize;

#[derive(Deserialize)]
struct CallToolRequest {
    name: String,
    arguments: Option<Value>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    limit: Option<u32>,
}

pub struct McpService {
    store: Arc<SqliteStore>,
}

impl McpService {
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self { store }
    }

    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let (read_tx, read_rx) = mpsc::channel::<String>(32);
        let (write_tx, mut write_rx) = mpsc::channel::<String>(32);

        // Stdin reader
        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let mut reader = BufReader::new(stdin).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if read_tx.send(line).await.is_err() {
                    break;
                }
            }
        });

        // Stdout writer
        tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            while let Some(msg) = write_rx.recv().await {
                let _ = stdout.write_all(msg.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
            }
        });

        let transport = StdioTransport::new(read_rx, write_tx);
        let server = Server::new(Arc::new(transport), Arc::new(self.clone()));
        server.start().await?;
        Ok(())
    }
}

impl Clone for McpService {
    fn clone(&self) -> Self {
        Self { store: self.store.clone() }
    }
}

#[derive(Deserialize)]
struct GraphArgs {
    uri: String,
    depth: Option<u32>,
}

#[async_trait]
impl ServerHandler for McpService {
    async fn initialize(
        &self, 
        _implementation: Implementation, 
        _capabilities: ClientCapabilities
    ) -> Result<ServerCapabilities, Error> {
        Ok(ServerCapabilities::default())
    }

    async fn shutdown(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn handle_method(&self, method: &str, params: Option<Value>) -> Result<Value, Error> {
        match method {
            "tools/list" => {
                let tools = vec![
                    Tool { 
                        name: "search_code".to_string(), 
                        description: "Search for code symbols by name".to_string(),
                        input_schema: serde_json::from_value(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "limit": { "type": "integer" }
                            },
                            "required": ["query"]
                        })).map_err(|e| Error::protocol(ErrorCode::ParseError, e.to_string()))?,
                        annotations: None,
                    },
                    Tool {
                        name: "get_callers".to_string(),
                        description: "Find callers of a function".to_string(),
                        input_schema: serde_json::from_value(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uri": { "type": "string" },
                                "depth": { "type": "integer" }
                            },
                            "required": ["uri"]
                        })).map_err(|e| Error::protocol(ErrorCode::ParseError, e.to_string()))?,
                        annotations: None,
                    },
                    Tool {
                        name: "get_callees".to_string(),
                        description: "Find callees of a function".to_string(),
                        input_schema: serde_json::from_value(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uri": { "type": "string" },
                                "depth": { "type": "integer" }
                            },
                            "required": ["uri"]
                        })).map_err(|e| Error::protocol(ErrorCode::ParseError, e.to_string()))?,
                        annotations: None,
                    },
                    Tool {
                        name: "get_impact".to_string(),
                        description: "Analyze impact of changes to a symbol".to_string(),
                        input_schema: serde_json::from_value(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uri": { "type": "string" },
                                "depth": { "type": "integer" }
                            },
                            "required": ["uri"]
                        })).map_err(|e| Error::protocol(ErrorCode::ParseError, e.to_string()))?,
                        annotations: None,
                    }
                ];
                let result = ListToolsResult { tools, next_cursor: None };
                serde_json::to_value(result).map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))
            },
            "tools/call" => {
                let req: CallToolRequest = params.and_then(|v| serde_json::from_value(v).ok())
                    .ok_or(Error::protocol(ErrorCode::InvalidParams, "Missing params"))?;
                
                let result_content = if req.name == "search_code" {
                    let args: SearchArgs = serde_json::from_value(req.arguments.unwrap_or(serde_json::json!({})))
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    
                    let engine = QueryEngine::new(&self.store);
                    let results = engine.search_by_name(&args.query, args.limit.unwrap_or(10) as usize)
                        .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                    
                    let mut text_output = String::new();
                    for res in results {
                         text_output.push_str(&format!("- {} ({})\n  {}\n", res.symbol.name, res.symbol.kind, res.symbol.uri.to_uri_string()));
                    }
                    if text_output.is_empty() {
                        text_output = "No results found.".to_string();
                    }
                    text_output

                } else if req.name == "get_callers" {
                    let args: GraphArgs = serde_json::from_value(req.arguments.unwrap_or(serde_json::json!({})))
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    
                    let engine = QueryEngine::new(&self.store);
                    let uri = crate::uri::SymbolUri::parse(&args.uri)
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    let callers = engine.find_callers(&uri, args.depth.unwrap_or(1) as usize)
                         .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                    
                    let mut text_output = String::new();
                    for symbol in callers {
                        text_output.push_str(&format!("- {} ({})\n  {}\n", symbol.name, symbol.kind, symbol.uri.to_uri_string()));
                    }
                    if text_output.is_empty() {
                        text_output = "No callers found.".to_string();
                    }
                    text_output

                } else if req.name == "get_callees" {
                    let args: GraphArgs = serde_json::from_value(req.arguments.unwrap_or(serde_json::json!({})))
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    
                    let engine = QueryEngine::new(&self.store);
                    let uri = crate::uri::SymbolUri::parse(&args.uri)
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    let callees = engine.find_callees(&uri, args.depth.unwrap_or(1) as usize)
                         .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                    
                    let mut text_output = String::new();
                    for symbol in callees {
                        text_output.push_str(&format!("- {} ({})\n  {}\n", symbol.name, symbol.kind, symbol.uri.to_uri_string()));
                    }
                    if text_output.is_empty() {
                        text_output = "No callees found.".to_string();
                    }
                    text_output

                } else if req.name == "get_impact" {
                    let args: GraphArgs = serde_json::from_value(req.arguments.unwrap_or(serde_json::json!({})))
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    
                    let engine = QueryEngine::new(&self.store);
                    let uri = crate::uri::SymbolUri::parse(&args.uri)
                        .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e.to_string()))?;
                    let impact = engine.impact_analysis(&uri, args.depth.unwrap_or(3) as usize)
                         .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                    
                    let mut text_output = String::new();
                    for res in impact {
                         let prefix = if res.is_direct() { "🔴 [DIRECT]" } else { "🟠 [INDIRECT]" };
                         text_output.push_str(&format!("{} {} ({})\n   {}\n", prefix, res.symbol.name, res.symbol.kind, res.symbol.uri.to_uri_string()));
                    }
                    if text_output.is_empty() {
                        text_output = "No impact found.".to_string();
                    }
                    text_output

                } else {
                    return Err(Error::protocol(ErrorCode::MethodNotFound, req.name));
                };

                // Create common response format
                let result = ToolResult {
                    content: Vec::new(),
                    structured_content: Some(serde_json::to_value(vec![
                        serde_json::json!({
                            "type": "text",
                            "text": result_content
                        })
                    ]).unwrap()),
                };
                
                serde_json::to_value(result).map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))
            },
            _ => Err(Error::protocol(ErrorCode::MethodNotFound, method.to_string()))
        }
    }
}
