use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agent::{AgentAccessReceipt, TypeDidEnvelope},
    did::DidDocument,
    lineage::{LineageAttestation, OpenLineageRunEvent},
    odrl::{Action, Policy},
    rbac::RbacPolicy,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QgLakeStoryReport {
    pub title: String,
    pub question: String,
    pub supervisor: QgLakeAgent,
    pub specialists: Vec<QgLakeSpecialistRun>,
    pub synthesis: QgLakeSynthesis,
    pub rbac: RbacPolicy,
    pub policies: Vec<Policy>,
    pub semantic_catalog: Vec<QgLakeSemanticDataset>,
    pub typesec: QgLakeTypeSecEvidence,
    pub open_lineage: OpenLineageRunEvent,
    pub did_attestation: LineageAttestation,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QgLakeAgent {
    pub name: String,
    pub role: String,
    pub did: DidDocument,
    pub seed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QgLakeSpecialistRun {
    pub agent: QgLakeAgent,
    pub compartment: String,
    pub raw_scope: Vec<String>,
    pub shared_output: String,
    pub request: TypeDidEnvelope,
    pub response: TypeDidEnvelope,
    pub access: AgentAccessReceipt,
    pub odrl_policy_id: String,
    pub croissant_dataset_id: String,
    pub cdif_dataset_id: String,
    pub summary: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QgLakeSynthesis {
    pub agent: QgLakeAgent,
    pub request: TypeDidEnvelope,
    pub access: AgentAccessReceipt,
    pub inputs: Vec<String>,
    pub briefing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QgLakeSemanticDataset {
    pub compartment: String,
    pub croissant: Value,
    pub cdif: Value,
    pub odrl: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QgLakeTypeSecEvidence {
    pub protocol: String,
    pub mode: String,
    pub envelope_count: usize,
    pub verified_delegate_capabilities: Vec<String>,
}

pub(crate) struct SpecialistSpec {
    pub(crate) name: &'static str,
    pub(crate) role: &'static str,
    pub(crate) compartment: &'static str,
    pub(crate) raw_scope: &'static [&'static str],
    pub(crate) shared_output: &'static str,
    pub(crate) rbac_action: &'static str,
    pub(crate) rbac_resource: &'static str,
    pub(crate) odrl_action: Action,
    pub(crate) allow_raw: bool,
    pub(crate) risk_signal: &'static str,
    pub(crate) evidence: &'static str,
}
