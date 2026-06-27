use serde_json::Value;

use super::model::QgLakeStoryReport;

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
        out.push('\n');
        if let Some(reason) = run.summary.get("denialReason").and_then(Value::as_str) {
            out.push_str("  Denial: ");
            out.push_str(reason);
            out.push('\n');
        }
        out.push_str("  ODRL policy: ");
        out.push_str(&run.odrl_policy_id);
        out.push_str("\n  TypeDID request hash: ");
        out.push_str(&run.request.payload_sha256);
        out.push_str("\n  TypeDID response hash: ");
        out.push_str(&run.response.payload_sha256);
        out.push('\n');
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
    out.push(' ');
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
