//! Agent2Agent (A2A) protocol surface.
//!
//! The agent runs already speak `typedid/a2a`; this module aligns that label
//! with the Linux Foundation A2A protocol by publishing an Agent Card at
//! `/.well-known/agent-card.json`. Skills mirror the `/v1` API, and the
//! security scheme documents the TypeDID envelope contract (Ed25519 under
//! `did:key` verification methods) shared with qg-python.

use serde_json::{Value, json};

/// A2A protocol version the card declares.
pub const A2A_PROTOCOL_VERSION: &str = "0.3.0";

/// Build the QueryGraph Agent Card for a deployment at `base_url`.
pub fn agent_card(base_url: &str) -> Value {
    let base = base_url.trim_end_matches('/');
    json!({
        "protocolVersion": A2A_PROTOCOL_VERSION,
        "name": "QueryGraph Navigator",
        "description": "Governed semantic-layer agent: builds four-layer semantic \
             bundles (Croissant, CDIF, DID, ODRL), answers over RBAC+ODRL-gated \
             lakehouse data with signed TypeDID envelopes, and emits an \
             OpenLineage evidence chain anchored by Ed25519 attestations.",
        "url": format!("{base}/v1"),
        "preferredTransport": "HTTP+JSON",
        "provider": {
            "organization": "QueryGraph",
            "url": "https://querygraph.ai",
        },
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": false,
        },
        "defaultInputModes": ["application/json"],
        "defaultOutputModes": ["application/json"],
        "skills": skills(),
        "securitySchemes": {
            "typedid": {
                "type": "http",
                "scheme": "bearer",
                "description": "TypeDID signed envelope: Ed25519 signature over \
                     the querygraph-typedid-signing-v1 payload, verifiable \
                     against the did:key verification method carried in the \
                     envelope. The sender must equal that signing DID. \
                     Denials are receipts, not errors.",
            },
        },
    })
}

/// The skill list is the cross-language contract: qg-python publishes the
/// same ids, names, and tags, asserted by the equivalence suite.
fn skills() -> Value {
    json!([
        {
            "id": "navigator-bundle",
            "name": "Build semantic bundle",
            "description": "Project a dataset into the four governed layers: Semantic Croissant, CDIF, DID, and ODRL.",
            "tags": ["semantic-layer", "croissant", "cdif", "did", "odrl"],
            "examples": ["Build a semantic bundle for the hazard vocabulary dataset."],
        },
        {
            "id": "qglake-story",
            "name": "Governed multi-agent run",
            "description": "Run the compartmentalized supervisor/specialist/broker/synthesis story with signed envelopes, policy receipts, and an OpenLineage evidence chain.",
            "tags": ["governance", "rbac", "odrl", "openlineage", "typedid"],
            "examples": ["Where do fiscal capacity, energy burden, mobility disruption, and climate-health risk overlap?"],
        },
        {
            "id": "verify-envelope",
            "name": "Verify TypeDID envelope",
            "description": "Verify a TypeDID envelope's payload hash and Ed25519 signature against its did:key verification method.",
            "tags": ["audit", "ed25519", "did", "typedid"],
            "examples": ["Verify this agent response envelope before trusting its summary."],
        },
        {
            "id": "import-semantic-model",
            "name": "Import semantic model",
            "description": "Import an OSI semantic model or a Semantic Croissant document into the governed model registry.",
            "tags": ["osi", "croissant", "semantic-layer"],
            "examples": ["Import this OSI YAML so agents can resolve its metrics."],
        },
        {
            "id": "semantic-search",
            "name": "Search semantic models",
            "description": "Find datasets, fields, metrics, and ontology terms matching a business term across registered semantic models.",
            "tags": ["search", "osi", "ontology"],
            "examples": ["Which fields describe monthly energy cost?"],
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_declares_protocol_url_and_skills() {
        let card = agent_card("http://localhost:8080/");
        assert_eq!(card["protocolVersion"], A2A_PROTOCOL_VERSION);
        assert_eq!(card["url"], "http://localhost:8080/v1");
        let ids: Vec<&str> = card["skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|skill| skill["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "navigator-bundle",
                "qglake-story",
                "verify-envelope",
                "import-semantic-model",
                "semantic-search",
            ]
        );
        assert!(
            card["securitySchemes"]["typedid"]["description"]
                .as_str()
                .unwrap()
                .contains("querygraph-typedid-signing-v1")
        );
    }
}
