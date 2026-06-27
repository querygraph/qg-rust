use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::verify::string_at;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatBootstrapManifest {
    pub schema_version: String,
    pub producer: String,
    pub standards: Vec<String>,
    pub table_artifacts: Vec<LakeCatTableArtifactHashes>,
    #[serde(default)]
    pub view_artifacts: Vec<LakeCatViewArtifactHashes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_hash: Option<String>,
    pub open_lineage_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub querygraph_import: Option<LakeCatQueryGraphImportCompatibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatTableArtifactHashes {
    pub stable_id: String,
    pub croissant_hash: String,
    pub cdif_hash: String,
    pub osi_hash: String,
    pub odrl_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_bindings_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatViewArtifactHashes {
    pub stable_id: String,
    pub osi_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatQueryGraphImportCompatibility {
    pub schema_version: String,
    pub table_only_bundle_hash: String,
    pub view_count: usize,
    pub graph_hash: String,
    #[serde(default)]
    pub view_receipt_evidence: Vec<LakeCatViewReceiptEvidence>,
    pub view_receipt_evidence_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatViewReceiptEvidence {
    pub stable_id: String,
    pub view_version: u64,
    pub receipt_hash: String,
    pub receipt_chain_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatTableProjection {
    pub ident: Value,
    pub stable_id: String,
    pub location: String,
    pub metadata_location: Option<String>,
    pub version: u64,
    pub format_version: Option<i64>,
    pub croissant: Value,
    pub cdif: Value,
    pub osi: Value,
    pub odrl: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_bindings: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatViewProjection {
    pub stable_id: String,
    pub warehouse: String,
    pub namespace: Vec<String>,
    pub name: String,
    pub view_version: u64,
    pub sql: String,
    pub dialect: String,
    pub schema_version: u64,
    pub columns: Value,
    pub properties: Value,
    pub osi: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatBootstrapVerification {
    pub warehouse: String,
    pub table_count: usize,
    pub view_count: usize,
    pub verified_tables: Vec<String>,
    pub verified_views: Vec<String>,
    pub bundle_hash: String,
    pub graph_hash: String,
    pub open_lineage_hash: String,
    pub querygraph_import_hash: Option<String>,
    pub standards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatImportPlan {
    pub verification: LakeCatBootstrapVerification,
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

impl From<&LakeCatTableProjection> for LakeCatImportTable {
    fn from(table: &LakeCatTableProjection) -> Self {
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

impl From<&LakeCatViewProjection> for LakeCatImportView {
    fn from(view: &LakeCatViewProjection) -> Self {
        Self {
            stable_id: view.stable_id.clone(),
            name: view.name.clone(),
            view_version: view.view_version,
            dialect: view.dialect.clone(),
            osi_model: string_at(&view.osi, &["view", "name"]),
        }
    }
}
