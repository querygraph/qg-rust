use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use typesec_integrations::{
    A2aTypeDidAdapter, Did, DidMessageBody, Ed25519DidKey, Ed25519DidKeyStore,
    SecureEnvelopeAdapter, StaticDidResolver, TypeDidGateway, TypeDidMode, TypeDidProfile,
    TypeDidWrapRequest,
};

use crate::{
    codata::AnchoredDid,
    dataverse::DataverseDataset,
    did::DidDocument,
    odrl::{Action, Policy},
    rbac::{RbacDecision, RbacPolicy},
    sail::SailLoadReport,
};

mod ollama;
pub use self::ollama::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeDidEnvelope {
    pub protocol: String,
    pub mode: String,
    pub conversation_id: String,
    pub sender: String,
    pub recipient: String,
    pub content_type: String,
    pub payload_sha256: String,
    pub signature: String,
    /// Audit-safe fields from TypeSec 0.10 "Murano"'s
    /// `VerifiedTypeDidMessage::attestation()`: who did what to which resource,
    /// at which privacy level, under which negotiated profile — without
    /// revealing the payload. The envelope digest binds the attestation to this
    /// exact wrapped message.
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub privacy: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub envelope_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentAccessReceipt {
    pub principal: String,
    pub action: String,
    pub allowed: bool,
    pub rbac: RbacDecision,
    pub odrl_allowed: bool,
    pub reason: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRunReport {
    pub request: TypeDidEnvelope,
    pub access: AgentAccessReceipt,
    pub selected_datasets: Vec<String>,
    pub sail_views: Vec<String>,
    pub prompt: String,
    pub ollama_reply: String,
    pub ollama_typedid: Option<TypeDidOllamaReport>,
    pub codata_anchor: Option<AnchoredDid>,
}

#[derive(Debug, Clone)]
pub struct QueryGraphAgent {
    pub agent_did: DidDocument,
    pub requester_did: DidDocument,
}

impl QueryGraphAgent {
    pub fn demo() -> Self {
        Self {
            agent_did: DidDocument::new_oyd("querygraph-agent", "QueryGraph Agent")
                .with_service_endpoint("http://localhost:8080/v1/answer"),
            requester_did: DidDocument::new_oyd("querygraph-requester", "TypeSec Demo Requester"),
        }
    }

    pub fn run_dataverse_answer(
        &self,
        question: &str,
        datasets: &[DataverseDataset],
        sail_report: &SailLoadReport,
        rbac: &RbacPolicy,
        policy: &Policy,
        codata_anchor: Option<AnchoredDid>,
    ) -> Result<AgentRunReport> {
        let selected_datasets = datasets
            .iter()
            .map(|dataset| dataset.persistent_id.clone())
            .collect::<Vec<_>>();
        let sail_views = sail_report
            .loads
            .iter()
            .flat_map(|load| {
                [
                    format!("{}_metadata", load.table_name),
                    format!("{}_files", load.table_name),
                ]
            })
            .collect::<Vec<_>>();
        let prompt = governed_prompt(question, datasets, &sail_views, sail_report);
        let payload = json!({
            "question": question,
            "prompt": prompt,
            "datasets": selected_datasets,
            "sailViews": sail_report.bootstrap_sql,
        });
        let request = TypeDidEnvelope::from_typesec(
            "querygraph.dataverse.answer",
            "application/vnd.querygraph.agent-request+json",
            &payload,
        )?;
        let rbac_decision = rbac.decide(&self.agent_did.id, "answer", "dataset");
        let odrl_allowed = policy.allows(&self.agent_did.id, &Action::Index)
            || policy.allows("public", &Action::Read);
        let allowed = rbac_decision.allowed && odrl_allowed;
        let access = AgentAccessReceipt {
            principal: self.agent_did.id.clone(),
            action: "answer".to_string(),
            allowed,
            rbac: rbac_decision,
            odrl_allowed,
            reason: if allowed {
                "TypeSec capabilities, RBAC role assignment, and ODRL policy allow the answer path"
                    .to_string()
            } else {
                "RBAC or ODRL policy denied the answer path".to_string()
            },
            checked_at: Utc::now(),
        };
        let ollama_reply = if allowed {
            format!(
                "I found {} governed Dataverse datasets staged in {} Sail views. The most relevant titles are: {}.",
                selected_datasets.len(),
                sail_views.len(),
                datasets
                    .iter()
                    .map(|dataset| dataset.title.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        } else {
            "Access denied by QueryGraph policy.".to_string()
        };

        Ok(AgentRunReport {
            request,
            access,
            selected_datasets,
            sail_views,
            prompt,
            ollama_reply,
            ollama_typedid: None,
            codata_anchor,
        })
    }
}

impl TypeDidEnvelope {
    pub fn from_typesec(
        conversation_id: &str,
        _content_type: &str,
        payload: &Value,
    ) -> Result<Self> {
        Self::from_typesec_between(
            conversation_id,
            "querygraph-dataverse-request-1",
            "querygraph/dataverse",
            b"querygraph-dataverse-requester",
            b"querygraph-dataverse-navigator",
            payload,
        )
    }

    pub fn from_typesec_between(
        conversation_id: &str,
        envelope_id: &str,
        resource: &str,
        sender_seed: impl AsRef<[u8]>,
        recipient_seed: impl AsRef<[u8]>,
        payload: &Value,
    ) -> Result<Self> {
        let planner_key = Ed25519DidKey::from_seed(sender_seed.as_ref());
        let navigator_key = Ed25519DidKey::from_seed(recipient_seed.as_ref());
        let planner = Did::key(planner_key.signing_public());
        let navigator = Did::key(navigator_key.signing_public());
        let resolver = StaticDidResolver::new()
            .with_document(planner_key.document(planner.clone()))
            .with_document(navigator_key.document(navigator.clone()));
        let key_store = Ed25519DidKeyStore::new()
            .with_key(planner.clone(), planner_key)
            .with_key(navigator.clone(), navigator_key);
        let profiles = vec![TypeDidProfile::ed25519_x25519_chacha20()];
        let payload_bytes = serde_json::to_vec(payload).expect("JSON payload should serialize");
        let adapter = A2aTypeDidAdapter;
        let envelope = adapter.wrap(
            TypeDidWrapRequest {
                id: envelope_id.to_string(),
                from: planner.clone(),
                to: navigator.clone(),
                conversation_id: conversation_id.to_string(),
                mode: TypeDidMode::RequestReply,
                body: DidMessageBody::agent_delegate(resource, "secret"),
                payload: &payload_bytes,
                local_profiles: &profiles,
                remote_profiles: &profiles,
            },
            &resolver,
            &key_store,
        )?;
        let gateway =
            TypeDidGateway::new(Arc::new(resolver), Arc::new(key_store), navigator.clone());
        let verified = gateway.open_message(&envelope)?;
        let attestation = verified.attestation();
        let payload_sha256 = hex_sha256(&payload_bytes);
        Ok(Self {
            protocol: format!("typedid/{}", verified.conversation.protocol),
            mode: match verified.conversation.mode {
                TypeDidMode::Send => "send".to_string(),
                TypeDidMode::RequestReply => "request-reply".to_string(),
            },
            conversation_id: verified.conversation.conversation_id,
            sender: envelope.from.as_str().to_string(),
            recipient: envelope
                .to
                .first()
                .map(|did| did.as_str().to_string())
                .unwrap_or_default(),
            content_type: adapter.content_type().to_string(),
            payload_sha256,
            signature: envelope.signature,
            action: attestation.action,
            resource: attestation.resource,
            privacy: attestation.privacy,
            profile: attestation.profile,
            envelope_digest: attestation.envelope_digest,
        })
    }
}

fn governed_prompt(
    question: &str,
    datasets: &[DataverseDataset],
    sail_views: &[String],
    sail_report: &SailLoadReport,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("Answer using only the governed Dataverse metadata below.\n");
    prompt.push_str("Question: ");
    prompt.push_str(question);
    prompt.push_str("\n\nDatasets:\n");
    for dataset in datasets {
        prompt.push_str("- ");
        prompt.push_str(&dataset.title);
        prompt.push_str(" (");
        prompt.push_str(&dataset.persistent_id);
        prompt.push_str("): ");
        prompt.push_str(&dataset.description);
        if !dataset.keywords.is_empty() {
            prompt.push_str(" Keywords: ");
            prompt.push_str(&dataset.keywords.join(", "));
            prompt.push('.');
        }
        if !dataset.subjects.is_empty() {
            prompt.push_str(" Subjects: ");
            prompt.push_str(&dataset.subjects.join(", "));
            prompt.push('.');
        }
        prompt.push('\n');
    }
    prompt.push_str("\nSail views: ");
    prompt.push_str(&sail_views.join(", "));
    if let Some(graph) = &sail_report.graph {
        prompt.push_str(&format!(
            "\nLive Sail graph loaded {} nodes and {} edges; verified {}.",
            graph.loaded_nodes,
            graph.loaded_edges,
            graph
                .verified_node_id
                .as_deref()
                .unwrap_or("no readback node")
        ));
    }
    prompt
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(crate) fn short_hex(bytes: &[u8]) -> String {
    hex_sha256(bytes).chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dataverse::sample_datasets,
        odrl::{Action, Rule},
        rbac::RbacRole,
        sail::LocalSailLakehouse,
    };

    #[test]
    fn agent_demo_produces_typedid_request_and_answer() {
        let datasets = sample_datasets();
        let root =
            std::env::temp_dir().join(format!("querygraph-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sail = LocalSailLakehouse::new(&root)
            .stage_dataverse_datasets(&datasets)
            .expect("staging should work");
        let agent = QueryGraphAgent::demo();
        let policy = Policy {
            id: "policy".to_string(),
            target: "dataset".to_string(),
            assigner: agent.agent_did.id.clone(),
            permissions: vec![Rule {
                action: Action::Index,
                assignee: agent.agent_did.id.clone(),
                constraint: None,
            }],
            prohibitions: vec![],
        };

        let report = agent
            .run_dataverse_answer(
                "Which datasets discuss access?",
                &datasets,
                &sail,
                &RbacPolicy::new()
                    .with_role(RbacRole::new("navigator").allow("answer", "dataset"))
                    .assign_role(agent.agent_did.id.clone(), "navigator"),
                &policy,
                None,
            )
            .expect("agent run should work");

        assert!(report.access.allowed);
        assert!(report.access.rbac.allowed);
        assert!(report.access.odrl_allowed);
        assert_eq!(report.request.mode, "request-reply");
        assert_eq!(report.request.protocol, "typedid/a2a");
        // Murano audit-safe attestation fields are surfaced on the envelope.
        assert_eq!(report.request.resource, "querygraph/dataverse");
        assert_eq!(report.request.privacy, "secret");
        assert!(!report.request.action.is_empty());
        assert!(!report.request.envelope_digest.is_empty());
        assert!(report.ollama_reply.contains("governed Dataverse datasets"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
