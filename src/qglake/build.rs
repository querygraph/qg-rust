use chrono::Utc;
use serde_json::{Value, json};

use crate::{
    croissant::{CroissantDataset, Field, FileObject, RecordSet},
    did::DidDocument,
    lineage::{OpenLineageRunEvent, bundle_hash, run_id_for},
    odrl::{Action, Policy, Rule},
    rbac::RbacDecision,
};

use super::model::{QgLakeAgent, QgLakeSpecialistRun, QgLakeSynthesis, SpecialistSpec};

pub(crate) fn qglake_agent(name: &str, role: &str) -> QgLakeAgent {
    let seed = format!("querygraph-qglake-{name}");
    QgLakeAgent {
        name: name.to_string(),
        role: role.to_string(),
        did: DidDocument::new_oyd(seed.as_bytes(), name)
            .with_service_endpoint(format!("qglake://agents/{role}")),
        seed,
    }
}

pub(crate) fn qglake_specs() -> Vec<SpecialistSpec> {
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

pub(crate) fn semantic_dataset(spec: &SpecialistSpec) -> CroissantDataset {
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

pub(crate) fn policy_for(
    supervisor: &QgLakeAgent,
    agent: &QgLakeAgent,
    spec: &SpecialistSpec,
) -> Policy {
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

pub(crate) fn openlineage_for_story(
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
            "runId": run_id_for(&hash),
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
