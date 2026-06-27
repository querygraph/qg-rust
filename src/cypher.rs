//! Crab-era Cypher reads over in-memory QueryGraph graphs.
//!
//! Grust 0.11.0 "Crab" added a standards-conformant GQL/Cypher language layer
//! with a portable Memory reference executor (`grust_cypher::read`). QueryGraph
//! uses it to interrogate the graphs it builds — the LakeCat catalog envelope
//! and the Dataverse semantic graph — with real `MATCH … RETURN` / `CALL`
//! queries instead of hand-rolled node/edge scans. Persistent backends push the
//! identical query down (see `grust::SailGraphStore::run_read_query`), so the
//! in-memory answers here are the same answers a live Sail warehouse returns.

use anyhow::{Result, anyhow};
use grust::prelude::*;
use grust_cypher::read::run_read_query;
use grust_cypher::{CypherParameters, CypherResultTable};
use serde_json::{Map, Value as JsonValue, json};

/// Run a read-only Cypher query against an in-memory grust graph (the Crab
/// Memory reference executor) and return the result table.
pub fn query_graph(graph: &Graph, cypher: &str) -> Result<CypherResultTable> {
    run_read_query(graph, cypher, &CypherParameters::new())
        .map_err(|err| anyhow!("Cypher query failed ({cypher}): {err}"))
}

/// Project a Cypher result table into a JSON object with named columns and
/// per-row maps, suitable for embedding in a QueryGraph report.
pub fn result_to_json(table: &CypherResultTable) -> JsonValue {
    let rows = table
        .rows
        .iter()
        .map(|row| {
            let mut obj = Map::new();
            for (column, value) in table.columns.iter().zip(row.iter()) {
                obj.insert(column.clone(), value.to_json());
            }
            JsonValue::Object(obj)
        })
        .collect::<Vec<_>>();
    json!({ "columns": table.columns, "rows": rows })
}

/// First-cell `i64` of a result table, e.g. for a `RETURN count(n)` query.
pub fn scalar_i64(table: &CypherResultTable) -> Option<i64> {
    match table.rows.first().and_then(|row| row.first()) {
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}

/// Distinct node labels in the graph via the `CALL db.labels()` catalog
/// procedure (a Crab-era read-only procedure).
pub fn labels(graph: &Graph) -> Result<Vec<String>> {
    let table = query_graph(graph, "CALL db.labels()")?;
    Ok(table
        .rows
        .iter()
        .filter_map(|row| row.first())
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect())
}

/// Count of nodes carrying a given label. `label` must be a Cypher label
/// identifier (caller supplies a known/derived label, never raw user input).
pub fn label_count(graph: &Graph, label: &str) -> Result<i64> {
    let table = query_graph(graph, &format!("MATCH (n:{label}) RETURN count(n) AS count"))?;
    Ok(scalar_i64(&table).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_graph() -> Graph {
        let mut builder = Graph::builder();
        let _ = builder
            .node("Dataset", "ds:1")
            .prop("title", "Hazard vocabulary")
            .finish();
        let _ = builder
            .node("Dataset", "ds:2")
            .prop("title", "Energy survey")
            .finish();
        let _ = builder.node("Field", "f:1").prop("name", "hazard").finish();
        let _ = builder.edge("has_field", "ds:1", "f:1").finish();
        builder.build()
    }

    #[test]
    fn runs_match_return_over_memory_reference() {
        let graph = sample_graph();
        let table =
            query_graph(&graph, "MATCH (d:Dataset) RETURN d.title AS title ORDER BY title")
                .expect("cypher read should run");
        assert_eq!(table.columns, vec!["title".to_string()]);
        assert_eq!(table.rows.len(), 2);
        let json = result_to_json(&table);
        assert_eq!(json["rows"][0]["title"], "Energy survey");
    }

    #[test]
    fn counts_nodes_by_label() {
        let graph = sample_graph();
        assert_eq!(label_count(&graph, "Dataset").expect("count"), 2);
        assert_eq!(label_count(&graph, "Field").expect("count"), 1);
    }

    #[test]
    fn lists_distinct_labels_via_catalog_procedure() {
        let graph = sample_graph();
        let mut labels = labels(&graph).expect("db.labels() should run");
        labels.sort();
        assert_eq!(labels, vec!["Dataset".to_string(), "Field".to_string()]);
    }
}
