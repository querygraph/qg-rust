use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

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

        Ok(LakeCatImportPlan {
            verification,
            graph_nodes: catalog_graph.node_count(),
            graph_edges: catalog_graph.edge_count(),
            tables: self.tables.iter().map(LakeCatImportTable::from).collect(),
            views: self.views.iter().map(LakeCatImportView::from).collect(),
        })
    }

    fn computed_bundle_hash(&self) -> Result<String> {
        content_hash_json(&serde_json::json!({
            "warehouse": self.warehouse,
            "manifest": self.manifest,
            "tables": self.tables,
            "views": self.views,
            "graph": self.graph,
            "openLineage": self.open_lineage,
        }))
    }

    fn computed_table_only_bundle_hash(&self) -> Result<String> {
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

fn validate_view_receipt_evidence(
    views: &[LakeCatViewProjection],
    evidence: &[LakeCatViewReceiptEvidence],
) -> Result<()> {
    if views.is_empty() {
        if evidence.is_empty() {
            return Ok(());
        }
        bail!("LakeCat QueryGraph import contract carries view receipt evidence without views");
    }
    if evidence.len() != views.len() {
        bail!(
            "LakeCat QueryGraph import contract lists {} view receipt evidence record(s) for {} view(s)",
            evidence.len(),
            views.len()
        );
    }
    for view in views {
        let Some(record) = evidence
            .iter()
            .find(|record| record.stable_id == view.stable_id)
        else {
            bail!(
                "LakeCat QueryGraph import contract is missing view receipt evidence for {}",
                view.stable_id
            );
        };
        if record.view_version != view.view_version {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} has version {}, expected {}",
                view.stable_id,
                record.view_version,
                view.view_version
            );
        }
        if record.receipt_hash.is_empty() {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} is missing a receipt hash",
                view.stable_id
            );
        }
        if record.receipt_chain_hash.is_empty() {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} is missing a receipt-chain hash",
                view.stable_id
            );
        }
    }
    Ok(())
}

fn validate_duplicate_free_stable_ids<'a>(
    label: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            bail!("{label} must be duplicate-free by stable id: {value}");
        }
    }
    Ok(())
}

fn table_only_artifact(artifact: &LakeCatTableArtifactHashes) -> Value {
    serde_json::json!({
        "stable-id": artifact.stable_id,
        "croissant-hash": artifact.croissant_hash,
        "cdif-hash": artifact.cdif_hash,
        "osi-hash": artifact.osi_hash,
        "odrl-hash": artifact.odrl_hash,
    })
}

fn table_only_projection(table: &LakeCatTableProjection) -> Value {
    serde_json::json!({
        "ident": table.ident,
        "stable-id": table.stable_id,
        "location": table.location,
        "metadata-location": table.metadata_location,
        "version": table.version,
        "format-version": table.format_version,
        "croissant": table.croissant,
        "cdif": table.cdif,
        "osi": table.osi,
        "odrl": table.odrl,
    })
}

fn assert_hash(label: &str, expected: &str, value: &Value) -> Result<()> {
    let actual = content_hash_json(value)?;
    if expected != actual {
        bail!("{label} hash mismatch: manifest={expected} actual={actual}");
    }
    Ok(())
}

fn content_hash_json(value: &Value) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("failed to encode JSON for LakeCat hash")?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToString::to_string)
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
    use serde_json::json;

    #[test]
    fn verifies_lakecat_bootstrap_manifest_hashes() {
        let croissant = json!({"@type":"cr:Dataset","name":"events"});
        let cdif = json!({"@type":"dcat:Dataset","dct:title":"events"});
        let osi = json!({"semantic_model":{"name":"events"}});
        let odrl = json!({"@type":"odrl:Policy","@id":"events#odrl"});
        let open_lineage = json!({"eventType":"COMPLETE"});
        let table = LakeCatTableProjection {
            ident: json!({
                "warehouse": "local",
                "namespace": ["default"],
                "name": "events"
            }),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: Some("file:///tmp/events/metadata/00000.json".to_string()),
            version: 0,
            format_version: Some(3),
            croissant: croissant.clone(),
            cdif: cdif.clone(),
            osi: osi.clone(),
            odrl: odrl.clone(),
            policy_bindings: Vec::new(),
        };
        let bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "pending".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: vec![
                    "Croissant".to_string(),
                    "CDIF".to_string(),
                    "OSI".to_string(),
                    "ODRL".to_string(),
                    "OpenLineage".to_string(),
                ],
                table_artifacts: vec![LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: content_hash_json(&croissant).unwrap(),
                    cdif_hash: content_hash_json(&cdif).unwrap(),
                    osi_hash: content_hash_json(&osi).unwrap(),
                    odrl_hash: content_hash_json(&odrl).unwrap(),
                    policy_bindings_hash: None,
                }],
                view_artifacts: Vec::new(),
                graph_hash: None,
                open_lineage_hash: content_hash_json(&open_lineage).unwrap(),
                querygraph_import: None,
            },
            tables: vec![table],
            views: Vec::new(),
            graph: json!({
                "nodes": [
                    {
                        "id": "lakecat:catalog:local",
                        "label": "Catalog",
                        "properties": {"warehouse": "local"}
                    },
                    {
                        "id": "lakecat:table:local:default:events",
                        "label": "Table",
                        "properties": {"stableId": "lakecat:table:local:default:events"}
                    }
                ],
                "edges": [
                    {
                        "from": "lakecat:catalog:local",
                        "to": "lakecat:table:local:default:events",
                        "label": "HAS_TABLE"
                    }
                ]
            }),
            open_lineage,
        };
        let bundle = LakeCatBootstrapBundle {
            bundle_hash: bundle.computed_bundle_hash().unwrap(),
            ..bundle
        };

        let verification = bundle.verify_manifest().unwrap();
        assert_eq!(verification.warehouse, "local");
        assert_eq!(verification.table_count, 1);
        assert_eq!(
            verification.verified_tables,
            vec!["lakecat:table:local:default:events"]
        );
        assert_eq!(verification.bundle_hash, bundle.bundle_hash);

        let plan = bundle.import_plan().unwrap();
        assert_eq!(plan.graph_nodes, 2);
        assert_eq!(plan.graph_edges, 1);
        assert_eq!(
            plan.tables[0].stable_id,
            "lakecat:table:local:default:events"
        );
        assert_eq!(plan.tables[0].croissant_name.as_deref(), Some("events"));
    }

    #[test]
    fn rejects_lakecat_bootstrap_manifest_hash_mismatch() {
        let mut bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "sha256:not-real".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: vec!["Croissant".to_string()],
                table_artifacts: vec![LakeCatTableArtifactHashes {
                    stable_id: "lakecat:table:local:default:events".to_string(),
                    croissant_hash: "sha256:not-real".to_string(),
                    cdif_hash: content_hash_json(&json!({})).unwrap(),
                    osi_hash: content_hash_json(&json!({})).unwrap(),
                    odrl_hash: content_hash_json(&json!({})).unwrap(),
                    policy_bindings_hash: None,
                }],
                view_artifacts: Vec::new(),
                graph_hash: None,
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
                querygraph_import: None,
            },
            tables: vec![LakeCatTableProjection {
                ident: json!({}),
                stable_id: "lakecat:table:local:default:events".to_string(),
                location: "file:///tmp/events".to_string(),
                metadata_location: None,
                version: 0,
                format_version: None,
                croissant: json!({"name":"events"}),
                cdif: json!({}),
                osi: json!({}),
                odrl: json!({}),
                policy_bindings: Vec::new(),
            }],
            views: Vec::new(),
            graph: json!({}),
            open_lineage: json!({}),
        };

        let err = bundle.verify_manifest().unwrap_err().to_string();
        assert!(err.contains("Croissant hash mismatch"));

        bundle.manifest.table_artifacts.clear();
        let err = bundle.verify_manifest().unwrap_err().to_string();
        assert!(err.contains("table artifact count"));
    }

    #[test]
    fn rejects_duplicate_lakecat_table_stable_ids() {
        let table = LakeCatTableProjection {
            ident: json!({}),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: None,
            version: 0,
            format_version: None,
            croissant: json!({}),
            cdif: json!({}),
            osi: json!({}),
            odrl: json!({}),
            policy_bindings: Vec::new(),
        };
        let bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "sha256:not-checked-before-identity-validation".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: Vec::new(),
                table_artifacts: vec![
                    LakeCatTableArtifactHashes {
                        stable_id: table.stable_id.clone(),
                        croissant_hash: "sha256:not-checked".to_string(),
                        cdif_hash: "sha256:not-checked".to_string(),
                        osi_hash: "sha256:not-checked".to_string(),
                        odrl_hash: "sha256:not-checked".to_string(),
                        policy_bindings_hash: None,
                    },
                    LakeCatTableArtifactHashes {
                        stable_id: table.stable_id.clone(),
                        croissant_hash: "sha256:not-checked".to_string(),
                        cdif_hash: "sha256:not-checked".to_string(),
                        osi_hash: "sha256:not-checked".to_string(),
                        odrl_hash: "sha256:not-checked".to_string(),
                        policy_bindings_hash: None,
                    },
                ],
                view_artifacts: Vec::new(),
                graph_hash: None,
                open_lineage_hash: "sha256:not-checked".to_string(),
                querygraph_import: None,
            },
            tables: vec![table.clone(), table],
            views: Vec::new(),
            graph: json!({}),
            open_lineage: json!({}),
        };

        let err = bundle.verify_manifest().unwrap_err().to_string();
        assert!(err.contains("LakeCat bootstrap table projections must be duplicate-free"));
    }

    #[test]
    fn rejects_lakecat_bootstrap_bundle_hash_mismatch() {
        let croissant = json!({"name":"events"});
        let table = LakeCatTableProjection {
            ident: json!({}),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: None,
            version: 0,
            format_version: None,
            croissant: croissant.clone(),
            cdif: json!({}),
            osi: json!({}),
            odrl: json!({}),
            policy_bindings: Vec::new(),
        };
        let bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "sha256:bad".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: vec!["Croissant".to_string()],
                table_artifacts: vec![LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: content_hash_json(&croissant).unwrap(),
                    cdif_hash: content_hash_json(&json!({})).unwrap(),
                    osi_hash: content_hash_json(&json!({})).unwrap(),
                    odrl_hash: content_hash_json(&json!({})).unwrap(),
                    policy_bindings_hash: None,
                }],
                view_artifacts: Vec::new(),
                graph_hash: None,
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
                querygraph_import: None,
            },
            tables: vec![table],
            views: Vec::new(),
            graph: json!({"nodes":[],"edges":[]}),
            open_lineage: json!({}),
        };

        let err = bundle.verify_manifest().unwrap_err().to_string();
        assert!(err.contains("bundle hash mismatch"));
    }

    #[test]
    fn rejects_lakecat_bootstrap_graph_with_invalid_edges() {
        let croissant = json!({"name":"events"});
        let table = LakeCatTableProjection {
            ident: json!({}),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: None,
            version: 0,
            format_version: None,
            croissant: croissant.clone(),
            cdif: json!({}),
            osi: json!({}),
            odrl: json!({}),
            policy_bindings: Vec::new(),
        };
        let bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "pending".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: vec!["Croissant".to_string()],
                table_artifacts: vec![LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: content_hash_json(&croissant).unwrap(),
                    cdif_hash: content_hash_json(&json!({})).unwrap(),
                    osi_hash: content_hash_json(&json!({})).unwrap(),
                    odrl_hash: content_hash_json(&json!({})).unwrap(),
                    policy_bindings_hash: None,
                }],
                view_artifacts: Vec::new(),
                graph_hash: None,
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
                querygraph_import: None,
            },
            tables: vec![table],
            views: Vec::new(),
            graph: json!({
                "nodes": [
                    {"id": "lakecat:catalog:local", "label": "Catalog"}
                ],
                "edges": [
                    {
                        "from": "lakecat:catalog:local",
                        "to": "lakecat:table:missing",
                        "label": "HAS_TABLE"
                    }
                ]
            }),
            open_lineage: json!({}),
        };
        let bundle = LakeCatBootstrapBundle {
            bundle_hash: bundle.computed_bundle_hash().unwrap(),
            ..bundle
        };

        let err = bundle.import_plan().unwrap_err().to_string();
        assert!(err.contains("LakeCat bootstrap graph validation failed"));
        assert!(err.contains("edge destination"));
    }

    #[test]
    fn verifies_view_bearing_lakecat_import_contract() {
        let croissant = json!({"name":"events"});
        let cdif = json!({"title":"events"});
        let table_osi = json!({"table":{"name":"events"}});
        let odrl = json!({"@id":"lakecat:policy:events"});
        let view_osi = json!({"view":{"name":"active_customers"}});
        let open_lineage = json!({"eventType":"COMPLETE"});
        let table = LakeCatTableProjection {
            ident: json!({
                "warehouse": "local",
                "namespace": ["default"],
                "name": "events"
            }),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: Some("file:///tmp/events/metadata/00000.json".to_string()),
            version: 1,
            format_version: Some(3),
            croissant: croissant.clone(),
            cdif: cdif.clone(),
            osi: table_osi.clone(),
            odrl: odrl.clone(),
            policy_bindings: vec![json!({"policy-id":"agent-read"})],
        };
        let view = LakeCatViewProjection {
            stable_id: "lakecat:view:local:default:active_customers".to_string(),
            warehouse: "local".to_string(),
            namespace: vec!["default".to_string()],
            name: "active_customers".to_string(),
            view_version: 1,
            sql: "select id from customers where active".to_string(),
            dialect: "sql".to_string(),
            schema_version: 1,
            columns: json!([{"name":"id","data-type":"int"}]),
            properties: json!({}),
            osi: view_osi.clone(),
        };
        let graph = json!({
            "nodes": [
                {"id": "lakecat:catalog", "label": "Catalog"},
                {"id": "lakecat:namespace:local:default", "label": "Namespace"},
                {"id": table.stable_id, "label": "IcebergTable"},
                {"id": view.stable_id, "label": "View"}
            ],
            "edges": [
                {
                    "from": "lakecat:catalog",
                    "to": "lakecat:namespace:local:default",
                    "label": "HAS_NAMESPACE"
                },
                {
                    "from": "lakecat:namespace:local:default",
                    "to": "lakecat:table:local:default:events",
                    "label": "CONTAINS_TABLE"
                },
                {
                    "from": "lakecat:namespace:local:default",
                    "to": "lakecat:view:local:default:active_customers",
                    "label": "CONTAINS_VIEW"
                }
            ]
        });
        let receipt_evidence = vec![LakeCatViewReceiptEvidence {
            stable_id: "lakecat:view:local:default:active_customers".to_string(),
            view_version: 1,
            receipt_hash: "sha256:view-version-receipt".to_string(),
            receipt_chain_hash: "sha256:view-receipt-chain".to_string(),
        }];
        let mut bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "pending".to_string(),
            manifest: LakeCatBootstrapManifest {
                schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
                producer: "https://querygraph.ai/lakecat".to_string(),
                standards: vec!["Iceberg REST".to_string(), "OpenLineage".to_string()],
                table_artifacts: vec![LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: content_hash_json(&croissant).unwrap(),
                    cdif_hash: content_hash_json(&cdif).unwrap(),
                    osi_hash: content_hash_json(&table_osi).unwrap(),
                    odrl_hash: content_hash_json(&odrl).unwrap(),
                    policy_bindings_hash: Some(
                        content_hash_json(&json!(table.policy_bindings)).unwrap(),
                    ),
                }],
                view_artifacts: vec![LakeCatViewArtifactHashes {
                    stable_id: view.stable_id.clone(),
                    osi_hash: content_hash_json(&view_osi).unwrap(),
                }],
                graph_hash: Some(content_hash_json(&graph).unwrap()),
                open_lineage_hash: content_hash_json(&open_lineage).unwrap(),
                querygraph_import: None,
            },
            tables: vec![table],
            views: vec![view],
            graph,
            open_lineage,
        };
        let receipt_evidence_hash =
            content_hash_json(&serde_json::to_value(&receipt_evidence).unwrap()).unwrap();
        let import = LakeCatQueryGraphImportCompatibility {
            schema_version: "lakecat.querygraph.import-compat.v1".to_string(),
            table_only_bundle_hash: bundle.computed_table_only_bundle_hash().unwrap(),
            view_count: 1,
            graph_hash: bundle.manifest.graph_hash.clone().unwrap(),
            view_receipt_evidence: receipt_evidence,
            view_receipt_evidence_hash: Some(receipt_evidence_hash),
        };
        bundle.manifest.querygraph_import = Some(import);
        bundle.bundle_hash = bundle.computed_bundle_hash().unwrap();

        let plan = bundle.import_plan().unwrap();
        assert_eq!(plan.verification.view_count, 1);
        assert_eq!(plan.views[0].stable_id, bundle.views[0].stable_id);
        assert_eq!(
            plan.verification.querygraph_import_hash.as_deref(),
            Some(
                bundle
                    .manifest
                    .querygraph_import
                    .as_ref()
                    .unwrap()
                    .table_only_bundle_hash
                    .as_str()
            )
        );
    }
}
