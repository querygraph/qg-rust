use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::dataverse::DataverseDataset;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiDocument {
    #[serde(default = "default_osi_version")]
    pub version: String,
    pub semantic_model: OsiSemanticModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiSemanticModel {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub ai_context: Option<String>,
    #[serde(default)]
    pub datasets: Vec<OsiDataset>,
    #[serde(default)]
    pub metrics: Vec<OsiMetric>,
    #[serde(default)]
    pub ontology_terms: Vec<OsiOntologyTerm>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiDataset {
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub ai_context: Option<String>,
    #[serde(default)]
    pub fields: Vec<OsiField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiField {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub semantic_type: Option<String>,
    #[serde(default)]
    pub expression: Option<OsiExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiMetric {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub expression: OsiExpression,
    #[serde(default)]
    pub ai_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiExpression {
    #[serde(default)]
    pub dialects: Vec<OsiDialectExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiDialectExpression {
    pub dialect: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsiOntologyTerm {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub source: Option<String>,
}

impl OsiDocument {
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self> {
        let yaml = fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&yaml)?)
    }

    pub fn for_dataverse(datasets: &[DataverseDataset]) -> Self {
        let osi_datasets = datasets
            .iter()
            .map(|dataset| OsiDataset {
                name: crate::sail::safe_sql_name(&dataset.title),
                source: format!(
                    "sail.{}",
                    crate::sail::safe_sql_name(&format!("dataverse_{}", dataset.id))
                ),
                description: Some(dataset.description.clone()),
                ai_context: Some(format!(
                    "Dataverse dataset {} with subjects {} and keywords {}.",
                    dataset.persistent_id,
                    dataset.subjects.join(", "),
                    dataset.keywords.join(", ")
                )),
                fields: vec![
                    OsiField {
                        name: "dataset_persistent_id".to_string(),
                        description: Some("Dataverse persistent dataset identifier.".to_string()),
                        semantic_type: Some("https://schema.org/identifier".to_string()),
                        expression: None,
                    },
                    OsiField {
                        name: "file_name".to_string(),
                        description: Some("Dataverse file name.".to_string()),
                        semantic_type: Some("https://schema.org/name".to_string()),
                        expression: None,
                    },
                    OsiField {
                        name: "download_url".to_string(),
                        description: Some("Dataverse file download URL.".to_string()),
                        semantic_type: Some("https://schema.org/contentUrl".to_string()),
                        expression: None,
                    },
                ],
            })
            .collect::<Vec<_>>();
        let ontology_terms = datasets
            .iter()
            .flat_map(|dataset| dataset.keywords.iter().chain(dataset.subjects.iter()))
            .map(|term| OsiOntologyTerm {
                id: format!("qg:ontology:{}", crate::sail::safe_sql_name(term)),
                label: term.clone(),
                source: Some("dataverse-citation-metadata".to_string()),
            })
            .collect::<Vec<_>>();

        Self {
            version: default_osi_version(),
            semantic_model: OsiSemanticModel {
                name: "querygraph_dataverse_navigator".to_string(),
                description: Some(
                    "Open Semantic Interchange model over Dataverse datasets staged in Sail."
                        .to_string(),
                ),
                ai_context: Some(
                    "Use dataset descriptions, subjects, keywords, and governed Sail views to answer."
                        .to_string(),
                ),
                datasets: osi_datasets,
                metrics: vec![OsiMetric {
                    name: "governed_dataset_count".to_string(),
                    description: Some("Number of governed Dataverse datasets in the Sail staging area.".to_string()),
                    expression: OsiExpression {
                        dialects: vec![OsiDialectExpression {
                            dialect: "SAIL_SQL".to_string(),
                            expression: "COUNT(DISTINCT dataset_persistent_id)".to_string(),
                        }],
                    },
                    ai_context: Some("Use this to summarize the governed dataset set available to the agent.".to_string()),
                }],
                ontology_terms,
            },
        }
    }

    /// Project a Semantic Croissant JSON-LD document into an OSI model,
    /// mirroring qg-python's `OsiDocument.from_croissant`: every recordSet
    /// field becomes a governed SAIL_SQL column expression, `sameAs` semantic
    /// types become ontology terms, and a `row_count` metric is attached.
    pub fn from_croissant_json(croissant: &serde_json::Value, sail_schema: &str) -> Result<Self> {
        let name = croissant["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("croissant document has no name"))?;
        let description = croissant["description"].as_str().map(str::to_string);
        let record_sets = croissant["recordSet"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let file_count = croissant["distribution"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();

        let mut fields = Vec::new();
        let mut ontology_terms = Vec::new();
        for record_set in &record_sets {
            for field in record_set["field"].as_array().into_iter().flatten() {
                let Some(field_name) = field["name"].as_str() else {
                    continue;
                };
                let semantic_type = field["sameAs"].as_str();
                fields.push(OsiField {
                    name: field_name.to_string(),
                    description: field["description"].as_str().map(str::to_string),
                    semantic_type: semantic_type.map(str::to_string),
                    expression: Some(OsiExpression {
                        dialects: vec![OsiDialectExpression {
                            dialect: "SAIL_SQL".to_string(),
                            expression: format!("`{field_name}`"),
                        }],
                    }),
                });
                if let Some(term) = semantic_type {
                    ontology_terms.push(OsiOntologyTerm {
                        id: term.to_string(),
                        label: field_name.to_string(),
                        source: Some("semantic-croissant".to_string()),
                    });
                }
            }
        }

        let safe_name = crate::sail::safe_sql_name(name);
        let field_count = fields.len();
        Ok(Self {
            version: default_osi_version(),
            semantic_model: OsiSemanticModel {
                name: format!("{safe_name}_semantic_model"),
                description: Some(format!(
                    "OSI model derived from Semantic Croissant dataset {name}."
                )),
                ai_context: Some(
                    "Resolve user intent to ontology terms, then map those terms \
                     to Croissant fields and governed Sail columns."
                        .to_string(),
                ),
                datasets: vec![OsiDataset {
                    name: safe_name.clone(),
                    source: format!("sail.{sail_schema}.{safe_name}"),
                    description,
                    ai_context: Some(format!(
                        "Dataset {name} has {file_count} file(s) and {field_count} semantic field(s)."
                    )),
                    fields,
                }],
                metrics: vec![OsiMetric {
                    name: "row_count".to_string(),
                    description: Some("Count of governed rows available in Sail.".to_string()),
                    expression: OsiExpression {
                        dialects: vec![OsiDialectExpression {
                            dialect: "SAIL_SQL".to_string(),
                            expression: "COUNT(*)".to_string(),
                        }],
                    },
                    ai_context: Some("Use this metric to verify loaded table scale.".to_string()),
                }],
                ontology_terms,
            },
        })
    }
}

fn default_osi_version() -> String {
    "0.2.0.dev0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataverse::sample_datasets;

    #[test]
    fn synthesizes_osi_from_dataverse() {
        let osi = OsiDocument::for_dataverse(&sample_datasets());
        assert_eq!(osi.semantic_model.name, "querygraph_dataverse_navigator");
        assert_eq!(osi.semantic_model.datasets.len(), 2);
        assert_eq!(osi.semantic_model.metrics[0].name, "governed_dataset_count");
        assert!(!osi.semantic_model.ontology_terms.is_empty());
    }

    #[test]
    fn parses_osi_yaml() {
        let yaml = r#"
version: 0.2.0.dev0
semantic_model:
  name: revenue
  datasets:
    - name: orders
      source: sail.orders
      fields:
        - name: amount
          semantic_type: https://schema.org/price
  metrics:
    - name: total_revenue
      expression:
        dialects:
          - dialect: SAIL_SQL
            expression: SUM(amount)
"#;
        let parsed: OsiDocument = serde_yaml::from_str(yaml).expect("valid OSI yaml");
        assert_eq!(parsed.semantic_model.metrics[0].name, "total_revenue");
    }
}
