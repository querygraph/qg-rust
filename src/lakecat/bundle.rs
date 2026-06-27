use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::*;
use super::verify::{
    assert_hash, content_hash_json, table_only_artifact, table_only_projection,
    validate_duplicate_free_stable_ids, validate_view_receipt_evidence,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatBootstrapBundle {
    pub warehouse: String,
    pub bundle_hash: String,
    pub manifest: LakeCatBootstrapManifest,
    pub tables: Vec<LakeCatTableProjection>,
    #[serde(default)]
    pub views: Vec<LakeCatViewProjection>,
    pub graph: Value,
    #[serde(rename = "open-lineage", alias = "openLineage")]
    pub open_lineage: Value,
}

impl LakeCatBootstrapBundle {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path).with_context(|| {
            format!("failed to read LakeCat bootstrap bundle {}", path.display())
        })?;
        serde_json::from_str(&data).with_context(|| {
            format!(
                "failed to parse LakeCat bootstrap bundle {}",
                path.display()
            )
        })
    }

    pub fn verify_manifest(&self) -> Result<LakeCatBootstrapVerification> {
        if self.manifest.schema_version != "lakecat.querygraph.bootstrap.v1" {
            bail!(
                "unsupported LakeCat bootstrap manifest schema version: {}",
                self.manifest.schema_version
            );
        }
        if self.manifest.table_artifacts.len() != self.tables.len() {
            bail!(
                "LakeCat manifest table artifact count {} does not match table count {}",
                self.manifest.table_artifacts.len(),
                self.tables.len()
            );
        }
        if self.manifest.view_artifacts.len() != self.views.len() {
            bail!(
                "LakeCat manifest view artifact count {} does not match view count {}",
                self.manifest.view_artifacts.len(),
                self.views.len()
            );
        }
        validate_duplicate_free_stable_ids(
            "LakeCat bootstrap table projections",
            self.tables.iter().map(|table| table.stable_id.as_str()),
        )?;
        validate_duplicate_free_stable_ids(
            "LakeCat bootstrap table artifacts",
            self.manifest
                .table_artifacts
                .iter()
                .map(|artifact| artifact.stable_id.as_str()),
        )?;
        validate_duplicate_free_stable_ids(
            "LakeCat bootstrap view projections",
            self.views.iter().map(|view| view.stable_id.as_str()),
        )?;
        validate_duplicate_free_stable_ids(
            "LakeCat bootstrap view artifacts",
            self.manifest
                .view_artifacts
                .iter()
                .map(|artifact| artifact.stable_id.as_str()),
        )?;

        let open_lineage_hash = content_hash_json(&self.open_lineage)?;
        if self.manifest.open_lineage_hash != open_lineage_hash {
            bail!(
                "LakeCat OpenLineage hash mismatch: manifest={} actual={}",
                self.manifest.open_lineage_hash,
                open_lineage_hash
            );
        }

        let mut verified_tables = Vec::with_capacity(self.tables.len());
        for table in &self.tables {
            let Some(artifact) = self
                .manifest
                .table_artifacts
                .iter()
                .find(|artifact| artifact.stable_id == table.stable_id)
            else {
                bail!(
                    "LakeCat manifest is missing table artifact hashes for {}",
                    table.stable_id
                );
            };

            assert_hash("Croissant", &artifact.croissant_hash, &table.croissant)?;
            assert_hash("CDIF", &artifact.cdif_hash, &table.cdif)?;
            assert_hash("OSI", &artifact.osi_hash, &table.osi)?;
            assert_hash("ODRL", &artifact.odrl_hash, &table.odrl)?;
            verified_tables.push(table.stable_id.clone());
        }

        let mut verified_views = Vec::with_capacity(self.views.len());
        for view in &self.views {
            let Some(artifact) = self
                .manifest
                .view_artifacts
                .iter()
                .find(|artifact| artifact.stable_id == view.stable_id)
            else {
                bail!(
                    "LakeCat manifest is missing view artifact hashes for {}",
                    view.stable_id
                );
            };
            assert_hash("View OSI", &artifact.osi_hash, &view.osi)?;
            verified_views.push(view.stable_id.clone());
        }

        let graph_hash = content_hash_json(&self.graph)?;
        if let Some(manifest_graph_hash) = self.manifest.graph_hash.as_deref()
            && manifest_graph_hash != graph_hash
        {
            bail!(
                "LakeCat graph hash mismatch: manifest={} actual={}",
                manifest_graph_hash,
                graph_hash
            );
        }
        if let Some(import) = self.manifest.querygraph_import.as_ref() {
            self.verify_querygraph_import(import, &graph_hash)?;
        }

        let bundle_hash = self.computed_bundle_hash()?;
        if self.bundle_hash != bundle_hash {
            bail!(
                "LakeCat bundle hash mismatch: manifest={} actual={}",
                self.bundle_hash,
                bundle_hash
            );
        }

        Ok(LakeCatBootstrapVerification {
            warehouse: self.warehouse.clone(),
            table_count: self.tables.len(),
            view_count: self.views.len(),
            verified_tables,
            verified_views,
            bundle_hash,
            graph_hash,
            open_lineage_hash,
            querygraph_import_hash: self
                .manifest
                .querygraph_import
                .as_ref()
                .map(|import| import.table_only_bundle_hash.clone()),
            standards: self.manifest.standards.clone(),
        })
    }

    fn verify_querygraph_import(
        &self,
        import: &LakeCatQueryGraphImportCompatibility,
        graph_hash: &str,
    ) -> Result<()> {
        if import.schema_version != "lakecat.querygraph.import-compat.v1" {
            bail!(
                "unsupported LakeCat QueryGraph import contract schema version: {}",
                import.schema_version
            );
        }
        let table_only_bundle_hash = self.computed_table_only_bundle_hash()?;
        if import.table_only_bundle_hash != table_only_bundle_hash {
            bail!(
                "LakeCat QueryGraph import hash mismatch: manifest={} actual={}",
                import.table_only_bundle_hash,
                table_only_bundle_hash
            );
        }
        if import.view_count != self.views.len() {
            bail!(
                "LakeCat QueryGraph import view count {} does not match bundle views {}",
                import.view_count,
                self.views.len()
            );
        }
        if import.graph_hash != graph_hash {
            bail!(
                "LakeCat QueryGraph import graph hash {} does not match bundle graph hash {}",
                import.graph_hash,
                graph_hash
            );
        }
        validate_view_receipt_evidence(&self.views, &import.view_receipt_evidence)?;
        if import.view_receipt_evidence.is_empty() {
            if import.view_receipt_evidence_hash.is_some() {
                bail!(
                    "LakeCat QueryGraph import contract has a receipt evidence hash without receipt evidence"
                );
            }
        } else {
            let evidence_hash =
                content_hash_json(&serde_json::to_value(&import.view_receipt_evidence)?)?;
            if import.view_receipt_evidence_hash.as_deref() != Some(evidence_hash.as_str()) {
                bail!(
                    "LakeCat QueryGraph import receipt evidence hash mismatch: manifest={:?} actual={}",
                    import.view_receipt_evidence_hash,
                    evidence_hash
                );
            }
        }
        Ok(())
    }

    pub fn import_plan(&self) -> Result<LakeCatImportPlan> {
        let verification = self.verify_manifest()?;
        let catalog_graph = grust::LakeCatCatalogGraph::from_json_value(&self.graph)
            .map_err(|err| anyhow!("LakeCat bootstrap graph validation failed: {err}"))?;

        // Interrogate the validated catalog graph with Grust 0.11 "Crab" Cypher
        // (the same query a live Sail backend would push down), rather than only
        // counting raw nodes/edges. `CALL db.labels()` enumerates the catalog's
        // node taxonomy, and a `MATCH (t:Table) RETURN count(t)` confirms the
        // table population the bundle claims.
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
            tables: self.tables.iter().map(LakeCatImportTable::from).collect(),
            views: self.views.iter().map(LakeCatImportView::from).collect(),
        })
    }

    pub(crate) fn computed_bundle_hash(&self) -> Result<String> {
        content_hash_json(&serde_json::json!({
            "warehouse": self.warehouse,
            "manifest": self.manifest,
            "tables": self.tables,
            "views": self.views,
            "graph": self.graph,
            "openLineage": self.open_lineage,
        }))
    }

    pub(crate) fn computed_table_only_bundle_hash(&self) -> Result<String> {
        content_hash_json(&serde_json::json!({
            "warehouse": self.warehouse,
            "manifest": {
                "schema-version": self.manifest.schema_version,
                "producer": self.manifest.producer,
                "standards": self.manifest.standards,
                "table-artifacts": self.manifest
                    .table_artifacts
                    .iter()
                    .map(table_only_artifact)
                    .collect::<Vec<_>>(),
                "open-lineage-hash": self.manifest.open_lineage_hash,
            },
            "tables": self
                .tables
                .iter()
                .map(table_only_projection)
                .collect::<Vec<_>>(),
            "graph": self.graph,
            "openLineage": self.open_lineage,
        }))
    }
}
