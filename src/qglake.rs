use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use typesec_core::{
    Capability,
    permissions::{AiCanInfer, CanReadSensitive},
    resource::GenericResource,
};

use crate::{
    agent::{AgentAccessReceipt, TypeDidEnvelope},
    cdif::CdifResource,
    croissant::{CroissantDataset, Field, FileObject, RecordSet},
    did::DidDocument,
    lineage::{LineageAttestation, OpenLineageRunEvent, bundle_hash},
    odrl::{Action, Policy, Rule},
    rbac::{RbacDecision, RbacPolicy, RbacRole},
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

struct SpecialistSpec {
    name: &'static str,
    role: &'static str,
    compartment: &'static str,
    raw_scope: &'static [&'static str],
    shared_output: &'static str,
    rbac_action: &'static str,
    rbac_resource: &'static str,
    odrl_action: Action,
    allow_raw: bool,
    risk_signal: &'static str,
    evidence: &'static str,
}

pub fn run_qglake_story() -> Result<QgLakeStoryReport> {
    let generated_at = Utc::now();
    let question = "Where do fiscal capacity, energy burden, mobility disruption, and climate-health risk overlap?".to_string();
    let supervisor = qglake_agent("SupervisorAgent", "supervisor");
    let synthesis_agent = qglake_agent("SynthesisAgent", "synthesis");
    let specs = qglake_specs();
    let mut rbac = RbacPolicy::new()
        .with_role(
            RbacRole::new("supervisor")
                .allow("delegate", "agents:*")
                .allow("aggregate", "summaries:*"),
        )
        .with_role(RbacRole::new("synthesis").allow("aggregate", "summaries:*"));
    for spec in &specs {
        rbac = rbac.with_role(RbacRole::new(spec.role).allow(spec.rbac_action, spec.rbac_resource));
    }
    rbac = rbac
        .assign_role(supervisor.did.id.clone(), "supervisor")
        .assign_role(synthesis_agent.did.id.clone(), "synthesis");

    let mut policies = Vec::new();
    let mut semantic_catalog = Vec::new();
    let mut runs = Vec::new();
    for spec in specs {
        let agent = qglake_agent(spec.name, spec.role);
        rbac = rbac.assign_role(agent.did.id.clone(), spec.role);
        let croissant = semantic_dataset(&spec);
        let cdif = CdifResource::from_croissant(
            &croissant,
            format!("https://querygraph.ai/qglake/{}", spec.compartment),
            format!("sail://qg_lakehouse/{}", spec.compartment.replace(':', "_")),
        );
        let policy = policy_for(&supervisor, &agent, &spec);
        let odrl_allowed = policy.allows(&agent.did.id, &spec.odrl_action);
        let rbac_decision = rbac.decide(&agent.did.id, spec.rbac_action, spec.rbac_resource);
        let allowed = rbac_decision.allowed && odrl_allowed && spec.allow_raw;
        let request_payload = json!({
            "question": question,
            "delegate": spec.shared_output,
            "compartment": spec.compartment,
            "rawScope": spec.raw_scope,
            "semanticCroissant": croissant.to_json_ld(),
            "cdif": cdif.to_json_ld(),
            "odrlPolicy": policy.to_json_ld()
        });
        let request = TypeDidEnvelope::from_typesec_between(
            &format!("qglake.{}.request", spec.compartment),
            &format!("qglake-{}-request", spec.name),
            spec.compartment,
            supervisor.seed.as_bytes(),
            agent.seed.as_bytes(),
            &request_payload,
        )?;
        let summary = if allowed {
            json!({
                "status": "allowed",
                "signal": spec.risk_signal,
                "evidence": spec.evidence,
                "sharedOutput": spec.shared_output,
                "redactions": ["raw rows", "direct identifiers", "unauthorized joins"]
            })
        } else {
            json!({
                "status": "denied",
                "signal": "metadata-only",
                "evidence": "Restricted raw access was denied; only CDIF/Croissant metadata may be shared.",
                "sharedOutput": spec.shared_output,
                "denialReason": "RBAC/ODRL permits metadata inspection but raw restricted data requires an external credential."
            })
        };
        let response = TypeDidEnvelope::from_typesec_between(
            &format!("qglake.{}.response", spec.compartment),
            &format!("qglake-{}-response", spec.name),
            "summaries:qglake",
            agent.seed.as_bytes(),
            synthesis_agent.seed.as_bytes(),
            &summary,
        )?;
        let access = AgentAccessReceipt {
            principal: agent.did.id.clone(),
            action: spec.rbac_action.to_string(),
            allowed,
            rbac: rbac_decision,
            odrl_allowed,
            reason: if allowed {
                "RBAC role, ODRL permission, and TypeDID delegation permit a compartment summary."
                    .to_string()
            } else {
                "Compartment policy returned a signed denial or metadata-only receipt.".to_string()
            },
            checked_at: generated_at,
        };

        semantic_catalog.push(QgLakeSemanticDataset {
            compartment: spec.compartment.to_string(),
            croissant: croissant.to_json_ld(),
            cdif: cdif.to_json_ld(),
            odrl: policy.to_json_ld(),
        });
        policies.push(policy.clone());
        runs.push(QgLakeSpecialistRun {
            agent,
            compartment: spec.compartment.to_string(),
            raw_scope: spec
                .raw_scope
                .iter()
                .map(|scope| scope.to_string())
                .collect(),
            shared_output: spec.shared_output.to_string(),
            request,
            response,
            access,
            odrl_policy_id: policy.id,
            croissant_dataset_id: croissant.id,
            cdif_dataset_id: cdif.dataset_id,
            summary,
        });
    }

    let synthesis_inputs = runs
        .iter()
        .map(|run| format!("{}:{}", run.agent.name, run.shared_output))
        .collect::<Vec<_>>();
    let synthesis_payload = json!({
        "question": question,
        "inputs": synthesis_inputs,
        "summaryHashes": runs.iter().map(|run| run.response.payload_sha256.clone()).collect::<Vec<_>>()
    });
    let synthesis_request = TypeDidEnvelope::from_typesec_between(
        "qglake.synthesis.request",
        "qglake-synthesis-request",
        "summaries:qglake",
        supervisor.seed.as_bytes(),
        synthesis_agent.seed.as_bytes(),
        &synthesis_payload,
    )?;
    let synthesis_rbac = rbac.decide(&synthesis_agent.did.id, "aggregate", "summaries:*");
    let synthesis_access = AgentAccessReceipt {
        principal: synthesis_agent.did.id.clone(),
        action: "aggregate".to_string(),
        allowed: synthesis_rbac.allowed,
        rbac: synthesis_rbac,
        odrl_allowed: true,
        reason: "Synthesis agent can aggregate signed summaries, not raw compartments.".to_string(),
        checked_at: generated_at,
    };
    let synthesis = QgLakeSynthesis {
        agent: synthesis_agent,
        request: synthesis_request,
        access: synthesis_access,
        inputs: synthesis_inputs,
        briefing: "Priority areas are those where weak fiscal capacity, energy burden, mobility fragility, and climate-health exposure overlap. Restricted health data contributed only a signed metadata/denial receipt, so the briefing uses approved compartment summaries rather than raw restricted rows.".to_string(),
    };
    let typesec = QgLakeTypeSecEvidence {
        protocol: "typedid/a2a".to_string(),
        mode: "request-reply".to_string(),
        envelope_count: runs.len() * 2 + 1,
        verified_delegate_capabilities: vec![
            Capability::<AiCanInfer, GenericResource>::permission_name().to_string(),
            Capability::<CanReadSensitive, GenericResource>::permission_name().to_string(),
            "CanDelegate".to_string(),
            "CanAggregateSummaries".to_string(),
            "CanReadDatasetCompartment".to_string(),
            "CanDeriveRedactedSummary".to_string(),
        ],
    };

    let lineage_event = openlineage_for_story(&question, &supervisor, &runs, &synthesis);
    let event_hash = lineage_event.event_hash();
    let did_attestation = LineageAttestation::from_event(
        supervisor.seed.clone(),
        "querygraph.qglake.story",
        &event_hash,
    )?;

    Ok(QgLakeStoryReport {
        title: "QGLake: The Resilience Desk".to_string(),
        question,
        supervisor,
        specialists: runs,
        synthesis,
        rbac,
        policies,
        semantic_catalog,
        typesec,
        open_lineage: lineage_event,
        did_attestation,
        generated_at,
    })
}

fn qglake_agent(name: &str, role: &str) -> QgLakeAgent {
    let seed = format!("querygraph-qglake-{name}");
    QgLakeAgent {
        name: name.to_string(),
        role: role.to_string(),
        did: DidDocument::new_oyd(seed.as_bytes(), name)
            .with_service_endpoint(format!("qglake://agents/{role}")),
        seed,
    }
}

pub fn render_qglake_story(report: &QgLakeStoryReport) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(&report.title);
    out.push_str("\n\nQuestion: ");
    out.push_str(&report.question);
    out.push_str("\n\n");
    out.push_str("A supervisor delegates the question into isolated compartments. Each specialist verifies TypeDID, RBAC, ODRL, Semantic Croissant, and CDIF before returning a signed summary. The synthesis agent aggregates summaries, not raw rows.\n\n");

    out.push_str("Supervisor\n");
    out.push_str("- ");
    out.push_str(&report.supervisor.name);
    out.push_str(" (");
    out.push_str(&report.supervisor.role);
    out.push_str(")\n");
    out.push_str("  DID: ");
    out.push_str(&report.supervisor.did.id);
    out.push_str("\n\n");

    out.push_str("Specialist Runs\n");
    for run in &report.specialists {
        let status = if run.access.allowed {
            "allowed"
        } else {
            "denied"
        };
        out.push_str("- ");
        out.push_str(&run.agent.name);
        out.push_str(" -> ");
        out.push_str(&run.shared_output);
        out.push_str(" [");
        out.push_str(status);
        out.push_str("]\n");
        out.push_str("  Compartment: ");
        out.push_str(&run.compartment);
        out.push_str("\n  Scope: ");
        out.push_str(&run.raw_scope.join(", "));
        out.push_str("\n  Signal: ");
        out.push_str(
            run.summary
                .get("signal")
                .and_then(Value::as_str)
                .unwrap_or("n/a"),
        );
        out.push_str("\n  Evidence: ");
        out.push_str(
            run.summary
                .get("evidence")
                .and_then(Value::as_str)
                .unwrap_or("n/a"),
        );
        out.push_str("\n");
        if let Some(reason) = run.summary.get("denialReason").and_then(Value::as_str) {
            out.push_str("  Denial: ");
            out.push_str(reason);
            out.push_str("\n");
        }
        out.push_str("  ODRL policy: ");
        out.push_str(&run.odrl_policy_id);
        out.push_str("\n  TypeDID request hash: ");
        out.push_str(&run.request.payload_sha256);
        out.push_str("\n  TypeDID response hash: ");
        out.push_str(&run.response.payload_sha256);
        out.push_str("\n");
    }

    out.push_str("\nSynthesis\n");
    out.push_str("- Agent: ");
    out.push_str(&report.synthesis.agent.name);
    out.push_str("\n- Inputs: ");
    out.push_str(&report.synthesis.inputs.join(", "));
    out.push_str("\n- Briefing: ");
    out.push_str(&report.synthesis.briefing);
    out.push_str("\n\n");

    out.push_str("Governance Evidence\n");
    out.push_str("- TypeDID: ");
    out.push_str(&report.typesec.protocol);
    out.push_str(" / ");
    out.push_str(&report.typesec.mode);
    out.push_str(", envelopes=");
    out.push_str(&report.typesec.envelope_count.to_string());
    out.push_str("\n- Capabilities: ");
    out.push_str(&report.typesec.verified_delegate_capabilities.join(", "));
    out.push_str("\n- Semantic Croissant/CDIF compartments: ");
    out.push_str(&report.semantic_catalog.len().to_string());
    out.push_str("\n- ODRL policies: ");
    out.push_str(&report.policies.len().to_string());
    out.push_str("\n- OpenLineage: ");
    out.push_str(&report.open_lineage.event_type);
    out.push_str(" ");
    out.push_str(
        report
            .open_lineage
            .run
            .get("runId")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    out.push_str("\n- DID attestation root: ");
    out.push_str(&report.did_attestation.merkle_root);
    out.push_str(
        "\n\nUse `cargo run -- qglake-story --json` for the full machine-readable report.\n",
    );
    out
}

fn qglake_specs() -> Vec<SpecialistSpec> {
    vec![
        SpecialistSpec {
            name: "FinanceAgent",
            role: "finance-specialist",
            compartment: "compartment:finance",
            raw_scope: &[
                "qg_lakehouse.government_finance__countydata",
                "qg_lakehouse.government_finance__municipaldata",
            ],
            shared_output: "fiscal-capacity summary",
            rbac_action: "read",
            rbac_resource: "compartment:finance",
            odrl_action: Action::Read,
            allow_raw: true,
            risk_signal: "low fiscal capacity and constrained district budgets",
            evidence: "County and municipal finance tables expose aggregate revenue, expenditure, and district capacity signals.",
        },
        SpecialistSpec {
            name: "EnergyAgent",
            role: "energy-specialist",
            compartment: "compartment:energy",
            raw_scope: &[
                "qg_lakehouse.energy_insecurity_covid__replication_survey",
                "qg_lakehouse.access_2018_energy__ceew_access2018",
            ],
            shared_output: "energy-burden summary",
            rbac_action: "summarize",
            rbac_resource: "compartment:energy",
            odrl_action: Action::Derive,
            allow_raw: true,
            risk_signal: "elevated household energy burden and access fragility",
            evidence: "Energy insecurity and ACCESS survey tables support aggregate burden indicators after redaction.",
        },
        SpecialistSpec {
            name: "MobilityAgent",
            role: "mobility-specialist",
            compartment: "compartment:mobility",
            raw_scope: &[
                "qg_lakehouse.dockless_transportation__ca_bg_level_prediction_data_v1",
                "qg_lakehouse.pedestrian_injury_ct__data_neutc_submit_ped_injury_severity_july_14_2025_xlsx",
            ],
            shared_output: "mobility-disruption summary",
            rbac_action: "summarize",
            rbac_resource: "compartment:mobility",
            odrl_action: Action::Derive,
            allow_raw: true,
            risk_signal: "mobility disruption near fragile corridors and pedestrian injury hot spots",
            evidence: "Dockless trip predictions, urban form, and injury severity features support corridor-level summaries.",
        },
        SpecialistSpec {
            name: "ClimateHealthAgent",
            role: "climate-health-specialist",
            compartment: "compartment:climate-health",
            raw_scope: &["qg_lakehouse.climate_health_pathways__imhdss_climate_mortality"],
            shared_output: "climate-health risk summary",
            rbac_action: "summarize",
            rbac_resource: "compartment:climate-health",
            odrl_action: Action::Derive,
            allow_raw: true,
            risk_signal: "heat and climate pathway exposure with mortality-sensitive periods",
            evidence: "Climate and mortality pathway tables support time-windowed aggregate health-risk signals.",
        },
        SpecialistSpec {
            name: "ReferenceAgent",
            role: "reference-specialist",
            compartment: "compartment:reference",
            raw_scope: &["qg_lakehouse.codata_constants_2022__codata_constants_2022"],
            shared_output: "unit and vocabulary normalization summary",
            rbac_action: "normalize",
            rbac_resource: "compartment:reference",
            odrl_action: Action::Read,
            allow_raw: true,
            risk_signal: "reference units and controlled vocabulary normalization are available",
            evidence: "CODATA constants provide a clean, typed reference-data table for units and measurement semantics.",
        },
        SpecialistSpec {
            name: "RestrictedDataBroker",
            role: "restricted-broker",
            compartment: "compartment:restricted:raw",
            raw_scope: &["qg_lakehouse.haalsi_baseline__restricted_raw"],
            shared_output: "restricted-data denial receipt",
            rbac_action: "read",
            rbac_resource: "compartment:restricted:metadata",
            odrl_action: Action::Read,
            allow_raw: false,
            risk_signal: "metadata-only",
            evidence: "HAALSI-like restricted data is catalog-visible but raw access requires an external credential.",
        },
    ]
}

fn semantic_dataset(spec: &SpecialistSpec) -> CroissantDataset {
    let dataset_id = format!("https://querygraph.ai/qglake/{}", spec.compartment);
    CroissantDataset {
        id: dataset_id.clone(),
        name: format!("QGLake {}", spec.shared_output),
        description: format!(
            "Semantic Croissant compartment for {} over {}.",
            spec.name,
            spec.raw_scope.join(", ")
        ),
        license: "Policy governed enterprise lakehouse access".to_string(),
        creators: vec!["QueryGraph".to_string(), spec.name.to_string()],
        files: spec
            .raw_scope
            .iter()
            .map(|scope| FileObject {
                id: format!("{dataset_id}/file/{scope}"),
                name: (*scope).to_string(),
                content_url: format!("sail://{scope}"),
                encoding_format: "application/vnd.apache.parquet".to_string(),
            })
            .collect(),
        record_sets: vec![RecordSet {
            id: format!("{dataset_id}/recordset/summary"),
            name: spec.shared_output.to_string(),
            fields: vec![
                Field::new(
                    "geography",
                    "sc:Text",
                    "Approved geographic aggregation key",
                )
                .semantic_type("https://schema.org/spatialCoverage"),
                Field::new("risk_signal", "sc:Text", "Compartment risk signal")
                    .semantic_type("https://schema.org/variableMeasured"),
                Field::new(
                    "policy_receipt",
                    "sc:Text",
                    "RBAC, ODRL, and TypeDID evidence",
                )
                .semantic_type("https://schema.org/conditionsOfAccess"),
            ],
        }],
        keywords: vec![
            "QueryGraph".to_string(),
            "QGLake".to_string(),
            spec.compartment.to_string(),
        ],
    }
}

fn policy_for(supervisor: &QgLakeAgent, agent: &QgLakeAgent, spec: &SpecialistSpec) -> Policy {
    Policy {
        id: format!("urn:qglake:policy:{}", spec.compartment.replace(':', "-")),
        target: spec.compartment.to_string(),
        assigner: supervisor.did.id.clone(),
        permissions: vec![Rule {
            action: spec.odrl_action.clone(),
            assignee: agent.did.id.clone(),
            constraint: Some(format!(
                "{} may produce {}; raw rows stay inside {}",
                agent.name, spec.shared_output, spec.compartment
            )),
        }],
        prohibitions: if spec.allow_raw {
            vec![Rule {
                action: Action::Use,
                assignee: agent.did.id.clone(),
                constraint: Some("No unrestricted export of raw rows.".to_string()),
            }]
        } else {
            vec![Rule {
                action: Action::Read,
                assignee: agent.did.id.clone(),
                constraint: Some(
                    "Raw restricted data requires external credential and explicit approval."
                        .to_string(),
                ),
            }]
        },
    }
}

fn openlineage_for_story(
    question: &str,
    supervisor: &QgLakeAgent,
    runs: &[QgLakeSpecialistRun],
    synthesis: &QgLakeSynthesis,
) -> OpenLineageRunEvent {
    let bundle = json!({
        "question": question,
        "supervisor": supervisor.did.id,
        "summaries": runs.iter().map(|run| &run.summary).collect::<Vec<_>>(),
        "synthesis": synthesis.briefing,
    });
    let hash = bundle_hash(&bundle);
    OpenLineageRunEvent {
        event_type: "COMPLETE".to_string(),
        event_time: Utc::now(),
        run: json!({
            "runId": format!("qglake-{}", &hash[..12]),
            "facets": {
                "queryGraph_typeDidHierarchy": {
                    "_producer": "https://querygraph.ai/qg-rust",
                    "_schemaURL": "https://querygraph.ai/schemas/openlineage/qglake-agent-hierarchy/0.1.0.json",
                    "supervisor": supervisor.did.id,
                    "specialists": runs.iter().map(|run| run.agent.did.id.clone()).collect::<Vec<_>>(),
                    "synthesis": synthesis.agent.did.id
                }
            }
        }),
        job: json!({
            "namespace": "querygraph.qglake",
            "name": "resilience-desk-supervised-agent-briefing"
        }),
        inputs: runs
            .iter()
            .flat_map(|run| {
                run.raw_scope.iter().map(|scope| {
                    json!({
                        "namespace": "sail",
                        "name": scope,
                        "facets": {
                            "queryGraph_compartment": {
                                "_producer": "https://querygraph.ai/qg-rust",
                                "_schemaURL": "https://querygraph.ai/schemas/openlineage/qglake-compartment/0.1.0.json",
                                "compartment": run.compartment,
                                "accessAllowed": run.access.allowed
                            }
                        }
                    })
                })
            })
            .collect(),
        outputs: vec![json!({
            "namespace": "querygraph",
            "name": "qglake.resilience-desk.briefing",
            "facets": {
                "queryGraph_signedSummaries": {
                    "_producer": "https://querygraph.ai/qg-rust",
                    "_schemaURL": "https://querygraph.ai/schemas/openlineage/qglake-signed-summaries/0.1.0.json",
                    "summaryPayloads": runs.iter().map(|run| run.response.payload_sha256.clone()).collect::<Vec<_>>(),
                    "briefing": synthesis.briefing
                }
            }
        })],
        producer: "https://querygraph.ai/qg-rust".to_string(),
        schema_url: "https://openlineage.io/spec/2-0-2/OpenLineage.json".to_string(),
    }
}

#[allow(dead_code)]
fn _assert_rbac_decision_is_serializable(decision: &RbacDecision) -> Value {
    serde_json::to_value(decision).expect("RBAC decision should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qglake_story_exercises_hierarchy_and_denial() {
        let report = run_qglake_story().expect("story should run");
        assert_eq!(report.specialists.len(), 6);
        assert!(
            report
                .specialists
                .iter()
                .all(|run| run.request.protocol == "typedid/a2a")
        );
        assert!(
            report
                .specialists
                .iter()
                .any(|run| run.agent.name == "RestrictedDataBroker" && !run.access.allowed)
        );
        assert!(
            report
                .specialists
                .iter()
                .filter(|run| run.access.allowed)
                .count()
                >= 5
        );
        assert_eq!(report.semantic_catalog.len(), 6);
        assert!(report.synthesis.access.allowed);
        assert_eq!(report.open_lineage.event_type, "COMPLETE");
        assert_eq!(
            report.did_attestation.event_hash,
            report.open_lineage.event_hash()
        );
    }

    #[test]
    fn qglake_story_renderer_is_readable() {
        let report = run_qglake_story().expect("story should run");
        let rendered = render_qglake_story(&report);

        assert!(rendered.contains("Specialist Runs"));
        assert!(rendered.contains("RestrictedDataBroker"));
        assert!(rendered.contains("Governance Evidence"));
        assert!(rendered.contains("qglake-story --json"));
        assert!(!rendered.trim_start().starts_with('{'));
    }
}
