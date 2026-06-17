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
            verified_tables,
            bundle_hash,
            open_lineage_hash,
            standards: self.manifest.standards.clone(),
        })
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
        })
    }

    fn computed_bundle_hash(&self) -> Result<String> {
        content_hash_json(&serde_json::json!({
            "warehouse": self.warehouse,
            "manifest": self.manifest,
            "tables": self.tables,
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
    pub open_lineage_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatTableArtifactHashes {
    pub stable_id: String,
    pub croissant_hash: String,
    pub cdif_hash: String,
    pub osi_hash: String,
    pub odrl_hash: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatBootstrapVerification {
    pub warehouse: String,
    pub table_count: usize,
    pub verified_tables: Vec<String>,
    pub bundle_hash: String,
    pub open_lineage_hash: String,
    pub standards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LakeCatImportPlan {
    pub verification: LakeCatBootstrapVerification,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub tables: Vec<LakeCatImportTable>,
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
                }],
                open_lineage_hash: content_hash_json(&open_lineage).unwrap(),
            },
            tables: vec![table],
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
                }],
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
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
            }],
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
                }],
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
            },
            tables: vec![table],
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
                }],
                open_lineage_hash: content_hash_json(&json!({})).unwrap(),
            },
            tables: vec![table],
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
}
