//! QueryGraph-side import-plan projections.
//!
//! The bootstrap-bundle wire format and its verification live in the shared
//! `qglake-bundle` crate (extracted in LakeCat 0.2.1 "Lynx" so the QueryGraph
//! importer consumes them instead of copying them). The types here are the
//! importer's own *output* — the acceptance handoff QueryGraph emits after
//! verifying a bundle.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use qglake_bundle::{
    QueryGraphBootstrapVerification, QueryGraphTableProjection, QueryGraphViewProjection,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatImportPlan {
    pub verification: QueryGraphBootstrapVerification,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    /// Distinct node labels in the catalog graph, from `CALL db.labels()`
    /// over the Grust 0.11 "Crab" Cypher reference executor.
    pub catalog_labels: Vec<String>,
    /// `Table` nodes in the catalog graph, from `MATCH (t:Table) RETURN count(t)`.
    pub table_count: i64,
    pub tables: Vec<LakeCatImportTable>,
    pub views: Vec<LakeCatImportView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatImportTable {
    pub stable_id: String,
    pub croissant_name: Option<String>,
    pub cdif_title: Option<String>,
    pub osi_model: Option<String>,
    pub odrl_policy: Option<String>,
}

impl From<&QueryGraphTableProjection> for LakeCatImportTable {
    fn from(table: &QueryGraphTableProjection) -> Self {
        Self {
            stable_id: table.stable_id.clone(),
            croissant_name: string_at(&table.croissant, &["name"]),
            cdif_title: string_at(&table.cdif, &["dct:title"]),
            osi_model: string_at(&table.osi, &["semantic_model", "name"]),
            odrl_policy: string_at(&table.odrl, &["@id"])
                .or_else(|| string_at(&table.odrl, &["uid"])),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatImportView {
    pub stable_id: String,
    pub name: String,
    pub view_version: u64,
    pub dialect: String,
    pub osi_model: Option<String>,
}

impl From<&QueryGraphViewProjection> for LakeCatImportView {
    fn from(view: &QueryGraphViewProjection) -> Self {
        Self {
            stable_id: view.stable_id.clone(),
            name: view.name.clone(),
            view_version: view.view_version,
            dialect: view.dialect.clone(),
            osi_model: string_at(&view.osi, &["view", "name"]),
        }
    }
}

/// Read a string from a nested JSON path (`["a", "b"]` -> `value["a"]["b"]`).
pub(crate) fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str().map(str::to_string)
}
