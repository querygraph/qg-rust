mod build;
mod model;
mod render;

pub use self::model::*;
pub use self::render::*;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use typesec_core::{
    Capability,
    permissions::{AiCanInfer, CanReadSensitive},
    resource::GenericResource,
};

use crate::{
    agent::{AgentAccessReceipt, TypeDidEnvelope},
    cdif::CdifResource,
    lineage::LineageAttestation,
    rbac::{RbacPolicy, RbacRole},
};

use self::build::{
    openlineage_for_story, policy_for, qglake_agent, qglake_specs, semantic_dataset,
};

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
