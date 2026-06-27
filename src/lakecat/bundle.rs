use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use qglake_bundle::{QueryGraphBootstrap, QueryGraphBootstrapVerification};

use super::model::{LakeCatImportPlan, LakeCatImportTable, LakeCatImportView};

/// A LakeCat QueryGraph bootstrap bundle, read for verification and import.
///
/// As of LakeCat 0.2.1 "Lynx" the bundle wire format and its verification are
/// the shared `qglake_bundle::QueryGraphBootstrap` contract — the same types
/// LakeCat produces — so QueryGraph verifies bundles with the catalog's own
/// code instead of a hand-maintained copy. This wrapper adds the QueryGraph-side
/// concerns: reading from disk and building a Cypher-enriched import plan.
#[derive(Debug, Clone, PartialEq)]
pub struct LakeCatBootstrapBundle(pub QueryGraphBootstrap);

impl LakeCatBootstrapBundle {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path).with_context(|| {
            format!("failed to read LakeCat bootstrap bundle {}", path.display())
        })?;
        let bootstrap = serde_json::from_str(&data).with_context(|| {
            format!(
                "failed to parse LakeCat bootstrap bundle {}",
                path.display()
            )
        })?;
        Ok(Self(bootstrap))
    }

    /// Verify the bundle against the shared `qglake-bundle` contract.
    pub fn verify_manifest(&self) -> Result<QueryGraphBootstrapVerification> {
        self.0
            .verify_manifest()
            .map_err(|err| anyhow!("LakeCat bootstrap verification failed: {err}"))
    }

    /// Verify the bundle, then interrogate the validated catalog graph with
    /// Grust 0.11 "Crab" Cypher to produce the QueryGraph acceptance handoff.
    pub fn import_plan(&self) -> Result<LakeCatImportPlan> {
        let verification = self.verify_manifest()?;

        // The bundle's typed catalog graph serializes to the `{nodes, edges}`
        // envelope Grust validates; load it, then query it the same way a live
        // Sail backend would push down: `CALL db.labels()` for the node taxonomy
        // and `MATCH (t:Table) RETURN count(t)` for the table population.
        let graph_value = serde_json::to_value(&self.0.graph)
            .with_context(|| "failed to encode LakeCat catalog graph")?;
        let catalog_graph = grust::LakeCatCatalogGraph::from_json_value(&graph_value)
            .map_err(|err| anyhow!("LakeCat bootstrap graph validation failed: {err}"))?;
        let graph = &catalog_graph.graph;
        let mut catalog_labels = crate::cypher::labels(graph)?;
        catalog_labels.sort();
        let table_count = crate::cypher::label_count(graph, "Table")?;

        Ok(LakeCatImportPlan {
            verification,
            graph_nodes: catalog_graph.node_count(),
            graph_edges: catalog_graph.edge_count(),
            catalog_labels,
            table_count,
            tables: self.0.tables.iter().map(LakeCatImportTable::from).collect(),
            views: self.0.views.iter().map(LakeCatImportView::from).collect(),
        })
    }
}
