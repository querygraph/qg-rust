use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
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

        Ok(LakeCatBootstrapVerification {
            warehouse: self.warehouse.clone(),
            table_count: self.tables.len(),
            verified_tables,
            open_lineage_hash,
            standards: self.manifest.standards.clone(),
        })
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
    pub stable_id: String,
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
    pub open_lineage_hash: String,
    pub standards: Vec<String>,
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
            stable_id: "lakecat:table:local:default:events".to_string(),
            croissant: croissant.clone(),
            cdif: cdif.clone(),
            osi: osi.clone(),
            odrl: odrl.clone(),
        };
        let bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "not-checked-by-importer-yet".to_string(),
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
            graph: json!({"nodes":[],"edges":[]}),
            open_lineage,
        };

        let verification = bundle.verify_manifest().unwrap();
        assert_eq!(verification.warehouse, "local");
        assert_eq!(verification.table_count, 1);
        assert_eq!(
            verification.verified_tables,
            vec!["lakecat:table:local:default:events"]
        );
    }

    #[test]
    fn rejects_lakecat_bootstrap_manifest_hash_mismatch() {
        let mut bundle = LakeCatBootstrapBundle {
            warehouse: "local".to_string(),
            bundle_hash: "unused".to_string(),
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
                stable_id: "lakecat:table:local:default:events".to_string(),
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
}
