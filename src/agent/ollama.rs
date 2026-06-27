use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use typesec_core::{
    Capability, PolicyEngine, ResourceId, SubjectId,
    permissions::{AiCanInfer, CanReadSensitive},
    policy::{PolicyResult, mint_capability},
    resource::GenericResource,
};
use typesec_integrations::{
    Did, DidEnvelope, DidMessageBody, DidMessageGateway, DidOllamaClient, Ed25519DidKey,
    Ed25519DidKeyStore, StaticDidResolver,
};

use super::short_hex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeDidOllamaReport {
    pub prompt_envelope_id: String,
    pub prompt_sender: String,
    pub prompt_recipient: String,
    pub prompt_resource: String,
    pub infer_capability: String,
    pub read_capability: String,
    pub reply_envelope_id: String,
    pub reply_sender: String,
    pub reply_recipient: String,
    pub reply_to: Option<String>,
    pub reply_signature: String,
}

#[derive(Debug, Clone)]
pub struct OllamaChatClient {
    base_url: String,
    model: String,
}

impl OllamaChatClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        }
    }

    pub fn chat(&self, prompt: &str) -> Result<String, reqwest::Error> {
        let response: Value = reqwest::blocking::Client::new()
            .post(format!("{}/api/chat", self.base_url))
            .json(&json!({
                "model": self.model,
                "stream": false,
                "messages": [{
                    "role": "user",
                    "content": prompt
                }]
            }))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

pub fn call_ollama_via_typedid(
    prompt: &str,
    base_url: impl Into<String>,
    model: impl Into<String>,
) -> Result<(String, TypeDidOllamaReport)> {
    let requester_key = Ed25519DidKey::from_seed(b"querygraph-typedid-ollama-requester");
    let gateway_key = Ed25519DidKey::from_seed(b"querygraph-typedid-ollama-gateway");
    let requester = Did::key(requester_key.signing_public());
    let gateway_did = Did::key(gateway_key.signing_public());
    let resolver = StaticDidResolver::new()
        .with_document(requester_key.document(requester.clone()))
        .with_document(gateway_key.document(gateway_did.clone()));
    let key_store = Ed25519DidKeyStore::new()
        .with_key(requester.clone(), requester_key)
        .with_key(gateway_did.clone(), gateway_key);
    let resource_id = format!(
        "querygraph/dataverse/prompt/{}",
        short_hex(prompt.as_bytes())
    );
    let prompt_envelope = DidEnvelope::prompt(
        "querygraph-dataverse-ollama-prompt-1",
        requester.clone(),
        gateway_did.clone(),
        DidMessageBody::infer_prompt(resource_id.clone()),
        prompt,
        &resolver,
        &key_store,
    )?;
    let gateway = DidMessageGateway::new(
        Arc::new(resolver.clone()),
        Arc::new(key_store.clone()),
        gateway_did.clone(),
    );
    let verified_prompt = gateway.open_prompt(&prompt_envelope)?;
    let policy = TypeDidOllamaPolicy {
        allowed_subject: requester.to_string(),
        allowed_resource: resource_id.clone(),
    };
    let infer: Capability<AiCanInfer, GenericResource> = mint_capability(
        &policy,
        verified_prompt.subject.as_str(),
        &verified_prompt.resource,
    )?;
    let read: Capability<CanReadSensitive, GenericResource> = mint_capability(
        &policy,
        verified_prompt.subject.as_str(),
        &verified_prompt.resource,
    )?;
    let ollama = DidOllamaClient::new(base_url, model);
    let reply_envelope = ollama.chat_verified_prompt_bound(
        verified_prompt,
        gateway_did.clone(),
        &resolver,
        &key_store,
        &infer,
        &read,
    )?;
    let requester_gateway =
        DidMessageGateway::new(Arc::new(resolver), Arc::new(key_store), requester.clone());
    let verified_reply = requester_gateway.open_prompt(&reply_envelope)?;
    let reply = verified_reply.prompt.reveal(&read)?;
    let reply_recipient = reply_envelope
        .to
        .first()
        .map(|did| did.as_str().to_string())
        .unwrap_or_default();
    let report = TypeDidOllamaReport {
        prompt_envelope_id: prompt_envelope.id,
        prompt_sender: requester.to_string(),
        prompt_recipient: gateway_did.to_string(),
        prompt_resource: resource_id,
        infer_capability: Capability::<AiCanInfer, GenericResource>::permission_name().to_string(),
        read_capability: Capability::<CanReadSensitive, GenericResource>::permission_name()
            .to_string(),
        reply_envelope_id: reply_envelope.id,
        reply_sender: reply_envelope.from.as_str().to_string(),
        reply_recipient,
        reply_to: reply_envelope
            .body
            .reply_to
            .as_ref()
            .map(|reference| reference.id.clone()),
        reply_signature: reply_envelope.signature,
    };
    Ok((reply, report))
}

struct TypeDidOllamaPolicy {
    allowed_subject: String,
    allowed_resource: String,
}

impl PolicyEngine for TypeDidOllamaPolicy {
    fn check(&self, subject: &SubjectId, action: &str, resource: &ResourceId) -> PolicyResult {
        if subject.as_str() == self.allowed_subject
            && resource.as_str() == self.allowed_resource
            && matches!(action, "ai:infer" | "read_sensitive")
        {
            PolicyResult::Allow
        } else {
            PolicyResult::Deny(format!(
                "{} may not {} {}",
                subject.as_str(),
                action,
                resource.as_str()
            ))
        }
    }
}
