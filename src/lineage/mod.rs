mod sail_sink;

pub use self::sail_sink::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use typesec_integrations::{Did, DidKeyStore, Ed25519DidKey, Ed25519DidKeyStore};

use crate::{agent::TypeDidEnvelope, dataverse::DataverseDataset, sail::SailLoadReport};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenLineageRunEvent {
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(rename = "eventTime")]
    pub event_time: DateTime<Utc>,
    pub run: Value,
    pub job: Value,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    pub producer: String,
    #[serde(rename = "schemaURL")]
    pub schema_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineageAttestation {
    pub issuer: String,
    pub subject: String,
    pub event_hash: String,
    pub merkle_root: String,
    pub signature_type: String,
    pub verification_method: String,
    pub signature: String,
    pub signed_payload_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenLineageEmission {
    pub target: String,
    pub event_hash: String,
    pub status: String,
    pub http_status: Option<u16>,
    pub path: Option<PathBuf>,
    pub emitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DidLedgerAppend {
    pub path: PathBuf,
    pub subject: String,
    pub merkle_root: String,
    pub signature: String,
    pub appended_at: DateTime<Utc>,
}

impl OpenLineageRunEvent {
    pub fn for_dataverse_agent_run(
        datasets: &[DataverseDataset],
        sail_report: &SailLoadReport,
        request: &TypeDidEnvelope,
        bundle_hash: &str,
    ) -> Self {
        let event_time = Utc::now();
        let run_id = format!(
            "querygraph-run-{}",
            short_hash(request.signature.as_bytes())
        );
        Self {
            event_type: "COMPLETE".to_string(),
            event_time,
            run: json!({
                "runId": run_id,
                "facets": {
                    "queryGraph_typeDid": {
                        "_producer": "https://querygraph.ai/qg-rust",
                        "_schemaURL": "https://querygraph.ai/schemas/openlineage/querygraph-typedid-facet/0.1.0.json",
                        "protocol": request.protocol,
                        "conversationId": request.conversation_id,
                        "payloadSha256": request.payload_sha256,
                        "signature": request.signature,
                    }
                }
            }),
            job: json!({
                "namespace": "querygraph.dataverse",
                "name": "dataverse-e2e-agent-answer",
                "facets": {
                    "queryGraph_semanticBundle": {
                        "_producer": "https://querygraph.ai/qg-rust",
                        "_schemaURL": "https://querygraph.ai/schemas/openlineage/querygraph-semantic-bundle-facet/0.1.0.json",
                        "bundleHash": bundle_hash,
                        "sailEndpoint": sail_report.graph.as_ref().map(|graph| graph.endpoint.clone()),
                        "loadedNodes": sail_report.graph.as_ref().map(|graph| graph.loaded_nodes),
                        "loadedEdges": sail_report.graph.as_ref().map(|graph| graph.loaded_edges),
                    }
                }
            }),
            inputs: datasets
                .iter()
                .map(|dataset| {
                    json!({
                        "namespace": "dataverse",
                        "name": dataset.persistent_id,
                        "facets": {
                            "dataSource": {
                                "_producer": "https://querygraph.ai/qg-rust",
                                "_schemaURL": "https://openlineage.io/spec/facets/1-0-0/DatasourceDatasetFacet.json",
                                "name": "Dataverse",
                                "uri": dataset.landing_page
                            }
                        }
                    })
                })
                .collect(),
            outputs: sail_report
                .loads
                .iter()
                .map(|load| {
                    json!({
                        "namespace": "sail",
                        "name": load.table_name,
                        "facets": {
                            "queryGraph_sailLoad": {
                                "_producer": "https://querygraph.ai/qg-rust",
                                "_schemaURL": "https://querygraph.ai/schemas/openlineage/querygraph-sail-load-facet/0.1.0.json",
                                "metadataPath": load.metadata_path,
                                "filesPath": load.files_path
                            }
                        }
                    })
                })
                .collect(),
            producer: "https://querygraph.ai/qg-rust".to_string(),
            schema_url: "https://openlineage.io/spec/2-0-2/OpenLineage.json".to_string(),
        }
    }

    pub fn event_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("OpenLineage event should serialize");
        sha256_hex(&bytes)
    }
}

impl LineageAttestation {
    pub fn from_event(
        issuer_seed: impl AsRef<str>,
        subject: impl Into<String>,
        event_hash: &str,
    ) -> anyhow::Result<Self> {
        let issuer_key = Ed25519DidKey::from_seed(issuer_seed.as_ref());
        let issuer_did = Did::key(issuer_key.signing_public());
        let key_store = Ed25519DidKeyStore::new().with_key(issuer_did.clone(), issuer_key);
        let issuer_document = key_store.document(&issuer_did)?;
        let verification_method = issuer_document
            .authentication
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("TypeSec DID document has no authentication key"))?;
        let subject = subject.into();
        let merkle_root = sha256_hex(format!("querygraph-lineage\n{event_hash}").as_bytes());
        let created_at = Utc::now();
        let signed_payload = signing_payload(
            issuer_did.as_str(),
            &subject,
            event_hash,
            &merkle_root,
            created_at,
        );
        let signature = key_store.sign(&issuer_did, signed_payload.as_bytes())?;
        let verification_key = issuer_document
            .verification_method
            .iter()
            .find(|method| method.id == verification_method)
            .ok_or_else(|| anyhow::anyhow!("TypeSec verification method is missing"))?;
        key_store.verify(verification_key, signed_payload.as_bytes(), &signature)?;
        Ok(Self {
            issuer: issuer_did.as_str().to_string(),
            subject,
            event_hash: event_hash.to_string(),
            merkle_root,
            signature_type: "Ed25519Signature2020".to_string(),
            verification_method,
            signature,
            signed_payload_sha256: sha256_hex(signed_payload.as_bytes()),
            created_at,
        })
    }
}

pub fn emit_openlineage_http(
    endpoint: impl AsRef<str>,
    event: &OpenLineageRunEvent,
) -> anyhow::Result<OpenLineageEmission> {
    let endpoint = endpoint.as_ref().to_string();
    let response = reqwest::blocking::Client::new()
        .post(&endpoint)
        .json(event)
        .send()?
        .error_for_status()?;
    Ok(OpenLineageEmission {
        target: endpoint,
        event_hash: event.event_hash(),
        status: "posted".to_string(),
        http_status: Some(response.status().as_u16()),
        path: None,
        emitted_at: Utc::now(),
    })
}

pub fn emit_openlineage_jsonl(
    path: impl AsRef<Path>,
    event: &OpenLineageRunEvent,
) -> anyhow::Result<OpenLineageEmission> {
    let path = path.as_ref();
    append_json_line(path, event)?;
    Ok(OpenLineageEmission {
        target: path.display().to_string(),
        event_hash: event.event_hash(),
        status: "appended".to_string(),
        http_status: None,
        path: Some(path.to_path_buf()),
        emitted_at: Utc::now(),
    })
}

pub fn append_did_ledger_attestation(
    path: impl AsRef<Path>,
    attestation: &LineageAttestation,
) -> anyhow::Result<DidLedgerAppend> {
    let path = path.as_ref();
    append_json_line(path, attestation)?;
    Ok(DidLedgerAppend {
        path: path.to_path_buf(),
        subject: attestation.subject.clone(),
        merkle_root: attestation.merkle_root.clone(),
        signature: attestation.signature.clone(),
        appended_at: Utc::now(),
    })
}

pub fn bundle_hash(bundle: &Value) -> String {
    let bytes = serde_json::to_vec(bundle).expect("bundle should serialize");
    sha256_hex(&bytes)
}

fn append_json_line(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn signing_payload(
    issuer: &str,
    subject: &str,
    event_hash: &str,
    merkle_root: &str,
    created_at: DateTime<Utc>,
) -> String {
    format!(
        "querygraph-lineage-attestation-v1\nissuer:{issuer}\nsubject:{subject}\nevent_hash:{event_hash}\nmerkle_root:{merkle_root}\ncreated_at:{}",
        created_at.to_rfc3339()
    )
}

fn short_hash(bytes: &[u8]) -> String {
    sha256_hex(bytes).chars().take(12).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agent::TypeDidEnvelope, dataverse::sample_datasets, sail::LocalSailLakehouse};

    #[test]
    fn builds_lineage_event_and_attestation() {
        let datasets = sample_datasets();
        let root =
            std::env::temp_dir().join(format!("querygraph-lineage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sail = LocalSailLakehouse::new(&root)
            .stage_dataverse_datasets(&datasets)
            .expect("staging should work");
        let request = TypeDidEnvelope::from_typesec(
            "querygraph.test",
            "application/json",
            &json!({"hello": "world"}),
        )
        .expect("typedid should wrap");

        let event = OpenLineageRunEvent::for_dataverse_agent_run(
            &datasets,
            &sail,
            &request,
            "sha256:bundle",
        );
        let hash = event.event_hash();
        let attestation =
            LineageAttestation::from_event("querygraph-lineage-test", "querygraph.test", &hash)
                .expect("TypeSec attestation should sign");

        assert_eq!(event.event_type, "COMPLETE");
        assert_eq!(attestation.event_hash, hash);
        assert_eq!(attestation.signature_type, "Ed25519Signature2020");
        assert!(attestation.issuer.starts_with("did:key:"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn emits_lineage_event_and_attestation_jsonl() {
        let datasets = sample_datasets();
        let root = std::env::temp_dir().join(format!(
            "querygraph-lineage-emit-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let sail = LocalSailLakehouse::new(root.join("sail"))
            .stage_dataverse_datasets(&datasets)
            .expect("staging should work");
        let request = TypeDidEnvelope::from_typesec(
            "querygraph.test",
            "application/json",
            &json!({"hello": "world"}),
        )
        .expect("typedid should wrap");
        let event = OpenLineageRunEvent::for_dataverse_agent_run(
            &datasets,
            &sail,
            &request,
            "sha256:bundle",
        );
        let attestation = LineageAttestation::from_event(
            "querygraph-lineage-test",
            "querygraph.test",
            &event.event_hash(),
        )
        .expect("attestation should sign");
        let event_path = root.join("openlineage/events.jsonl");
        let ledger_path = root.join("did-ledger/attestations.jsonl");

        let emission = emit_openlineage_jsonl(&event_path, &event).expect("event should append");
        let ledger = append_did_ledger_attestation(&ledger_path, &attestation)
            .expect("ledger should append");

        assert_eq!(emission.status, "appended");
        assert_eq!(ledger.merkle_root, attestation.merkle_root);
        assert!(
            std::fs::read_to_string(event_path)
                .expect("event file should read")
                .contains("\"eventType\":\"COMPLETE\"")
        );
        assert!(
            std::fs::read_to_string(ledger_path)
                .expect("ledger file should read")
                .contains("Ed25519Signature2020")
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
