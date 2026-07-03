//! Model Context Protocol server over stdio.
//!
//! A dependency-free JSON-RPC 2.0 implementation of the MCP server handshake
//! (`initialize` → `notifications/initialized` → `tools/list` /
//! `tools/call`), exposing the same governed surface as the `/v1` HTTP API
//! and qg-python's FastMCP server. One line in, one line out — pointable at
//! any MCP client (Claude Code/Desktop, LangChain, PydanticAI, …) via
//! `querygraph mcp-serve`.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

use anyhow::Result;
use serde_json::{Value, json};

use crate::{
    agent::PyTypeDidEnvelope,
    navigator::{AiNavigator, NavigatorInput},
    osi::OsiDocument,
    qglake::run_qglake_story,
    server::{answer_over_models, search_model},
};

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Stateful MCP session: the semantic-model registry persists across calls.
#[derive(Default)]
pub struct McpServer {
    registry: BTreeMap<String, OsiDocument>,
}

impl McpServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve MCP over stdio until EOF.
    pub fn run_stdio(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle_line(&line) {
                stdout.write_all(response.as_bytes())?;
                stdout.write_all(b"\n")?;
                stdout.flush()?;
            }
        }
        Ok(())
    }

    /// Handle one JSON-RPC message; `None` for notifications (no reply).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0", "id": null,
                        "error": {"code": -32700, "message": format!("parse error: {error}")},
                    })
                    .to_string(),
                );
            }
        };
        let id = message.get("id").cloned();
        let method = message["method"].as_str().unwrap_or_default().to_string();
        let params = message.get("params").cloned().unwrap_or(json!({}));

        // Notifications (no id) get no response.
        let id = match id {
            Some(id) if !id.is_null() => id,
            _ => return None,
        };

        let response = match self.dispatch(&method, &params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": error.code, "message": error.message},
            }),
        };
        Some(response.to_string())
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {
                    "name": "querygraph",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "QueryGraph governed semantic layer: import and \
                     search semantic models, build four-layer bundles, run the \
                     governed multi-agent story, answer questions with signed \
                     evidence chains, and verify TypeDID envelopes.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => self.call_tool(params),
            _ => Err(RpcError {
                code: -32601,
                message: format!("method not found: {method}"),
            }),
        }
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, RpcError> {
        let name = params["name"].as_str().unwrap_or_default();
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
        let result = match name {
            "build_navigator_bundle" => {
                let input: NavigatorInput = serde_json::from_value(arguments)
                    .map_err(|error| RpcError::invalid_params(&error.to_string()))?;
                Ok(AiNavigator.build(input).bundle)
            }
            "run_qglake_story" => run_qglake_story()
                .and_then(|report| Ok(serde_json::to_value(report)?))
                .map_err(|error| RpcError::internal(&error.to_string())),
            "verify_envelope" => {
                serde_json::from_value::<PyTypeDidEnvelope>(arguments["envelope"].clone())
                    .map(|envelope| {
                        serde_json::to_value(envelope.verify()).expect("report serializes")
                    })
                    .map_err(|error| RpcError::invalid_params(&error.to_string()))
            }
            "import_semantic_model" => {
                let document = if arguments.get("osi").is_some() {
                    serde_json::from_value::<OsiDocument>(arguments["osi"].clone())
                        .map_err(|error| RpcError::invalid_params(&error.to_string()))?
                } else if arguments.get("croissant").is_some() {
                    OsiDocument::from_croissant_json(&arguments["croissant"], "qg_lakehouse")
                        .map_err(|error| RpcError::invalid_params(&error.to_string()))?
                } else {
                    return Err(RpcError::invalid_params(
                        "pass either an 'osi' document or a 'croissant' JSON-LD document",
                    ));
                };
                let model = &document.semantic_model;
                let summary = json!({
                    "imported": model.name,
                    "datasets": model.datasets.len(),
                    "metrics": model.metrics.len(),
                });
                self.registry.insert(model.name.clone(), document);
                Ok(summary)
            }
            "search_semantic_models" => {
                let needle = arguments["term"]
                    .as_str()
                    .unwrap_or_default()
                    .to_lowercase();
                let matches: Vec<Value> = self
                    .registry
                    .values()
                    .flat_map(|document| search_model(&document.semantic_model, &needle))
                    .collect();
                Ok(json!({"term": needle, "matches": matches}))
            }
            "answer_question" => {
                let question = arguments["question"].as_str().unwrap_or_default();
                let models: Vec<OsiDocument> = self.registry.values().cloned().collect();
                answer_over_models(&models, question)
                    .map_err(|error| RpcError::internal(&error.to_string()))
            }
            other => Err(RpcError::invalid_params(&format!("unknown tool: {other}"))),
        };
        match result {
            Ok(value) => Ok(json!({
                "content": [{"type": "text", "text": value.to_string()}],
                "structuredContent": value,
                "isError": false,
            })),
            Err(error) => Ok(json!({
                "content": [{"type": "text", "text": error.message}],
                "isError": true,
            })),
        }
    }
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: message.to_string(),
        }
    }

    fn internal(message: &str) -> Self {
        Self {
            code: -32603,
            message: message.to_string(),
        }
    }
}

fn tool_definitions() -> Value {
    let string = |description: &str| json!({"type": "string", "description": description});
    json!([
        {
            "name": "build_navigator_bundle",
            "description": "Build the four-layer semantic bundle: Croissant, CDIF, DID, ODRL.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dataset_name": string("Dataset name"),
                    "description": string("Dataset description"),
                    "landing_page": string("Landing page URL"),
                    "data_url": string("Data file URL"),
                    "creator": string("Creator name"),
                    "agent_name": string("Agent name"),
                },
                "required": ["dataset_name", "description", "landing_page", "data_url", "creator", "agent_name"],
            },
        },
        {
            "name": "run_qglake_story",
            "description": "Run the governed multi-agent Resilience Desk story with the full evidence chain.",
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "verify_envelope",
            "description": "Verify a TypeDID envelope's payload hash and Ed25519 signature.",
            "inputSchema": {
                "type": "object",
                "properties": {"envelope": {"type": "object", "description": "TypeDID envelope JSON"}},
                "required": ["envelope"],
            },
        },
        {
            "name": "import_semantic_model",
            "description": "Import an OSI semantic model or a Semantic Croissant JSON-LD document into the registry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "osi": {"type": "object", "description": "OSI document"},
                    "croissant": {"type": "object", "description": "Croissant JSON-LD document"},
                },
            },
        },
        {
            "name": "search_semantic_models",
            "description": "Find datasets, fields, metrics, and ontology terms matching a term.",
            "inputSchema": {
                "type": "object",
                "properties": {"term": string("Business term or synonym")},
                "required": ["term"],
            },
        },
        {
            "name": "answer_question",
            "description": "Answer over registered semantic models: search, plan SQL, synthesize, sign, and attach the OpenLineage run.",
            "inputSchema": {
                "type": "object",
                "properties": {"question": string("Natural-language question")},
                "required": ["question"],
            },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &mut McpServer, message: Value) -> Value {
        let response = server
            .handle_line(&message.to_string())
            .expect("request gets a response");
        serde_json::from_str(&response).expect("response is JSON")
    }

    #[test]
    fn handshake_lists_tools_and_answers_over_imported_models() {
        let mut server = McpServer::new();

        let init = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                   "params": {"protocolVersion": MCP_PROTOCOL_VERSION, "capabilities": {}}}),
        );
        assert_eq!(init["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(init["result"]["serverInfo"]["name"], "querygraph");

        // The initialized notification gets no response.
        assert!(
            server
                .handle_line(
                    &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string()
                )
                .is_none()
        );

        let tools = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        );
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"answer_question"));
        assert!(names.contains(&"import_semantic_model"));

        let imported = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "import_semantic_model", "arguments": {"croissant": {
                "name": "Energy Burden",
                "recordSet": [{"field": [{"name": "monthly_cost"}]}],
            }}}}),
        );
        assert_eq!(
            imported["result"]["structuredContent"]["imported"],
            "energy_burden_semantic_model"
        );

        let answer = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                   "params": {"name": "answer_question",
                              "arguments": {"question": "what drives monthly energy cost?"}}}),
        );
        let structured = &answer["result"]["structuredContent"];
        assert_eq!(structured["plans"][0]["dataset"], "energy_burden");
        assert!(
            !structured["envelope"]["signature"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unknown_method_and_unknown_tool_are_reported() {
        let mut server = McpServer::new();
        let missing = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 1, "method": "resources/list"}),
        );
        assert_eq!(missing["error"]["code"], -32601);

        let unknown_tool = call(
            &mut server,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                   "params": {"name": "no_such_tool", "arguments": {}}}),
        );
        assert_eq!(unknown_tool["result"]["isError"], true);
    }
}
