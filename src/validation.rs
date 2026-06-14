use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lakehouse::LakehouseReport;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticValidationReport {
    pub report_path: PathBuf,
    pub datasets: usize,
    pub croissant_documents: usize,
    pub cdif_documents: usize,
    pub openlineage_documents: usize,
    pub issues: Vec<SemanticValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticValidationIssue {
    pub path: PathBuf,
    pub kind: String,
    pub message: String,
}

impl SemanticValidationReport {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_lakehouse_semantics(
    report_path: impl AsRef<Path>,
    openlineage_jsonl: Option<impl AsRef<Path>>,
) -> Result<SemanticValidationReport> {
    let report_path = report_path.as_ref().to_path_buf();
    let report: LakehouseReport = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let mut validation = SemanticValidationReport {
        report_path,
        datasets: report.datasets.len(),
        croissant_documents: 0,
        cdif_documents: 0,
        openlineage_documents: 0,
        issues: Vec::new(),
    };

    for dataset in &report.datasets {
        validate_json_file(
            &mut validation,
            &dataset.croissant_path,
            "croissant",
            validate_croissant,
        );
        validate_json_file(&mut validation, &dataset.cdif_path, "cdif", validate_cdif);
    }

    if let Some(openlineage_jsonl) = openlineage_jsonl {
        let path = openlineage_jsonl.as_ref();
        match fs::read_to_string(path) {
            Ok(lines) => {
                for (idx, line) in lines.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<Value>(line) {
                        Ok(value) => {
                            validation.openlineage_documents += 1;
                            for message in validate_openlineage(&value) {
                                validation.issues.push(SemanticValidationIssue {
                                    path: path.to_path_buf(),
                                    kind: "openlineage".to_string(),
                                    message: format!("line {}: {message}", idx + 1),
                                });
                            }
                        }
                        Err(err) => validation.issues.push(SemanticValidationIssue {
                            path: path.to_path_buf(),
                            kind: "openlineage".to_string(),
                            message: format!("line {}: invalid JSON: {err}", idx + 1),
                        }),
                    }
                }
            }
            Err(err) => validation.issues.push(SemanticValidationIssue {
                path: path.to_path_buf(),
                kind: "openlineage".to_string(),
                message: format!("cannot read OpenLineage JSONL: {err}"),
            }),
        }
    }

    Ok(validation)
}

fn validate_json_file(
    report: &mut SemanticValidationReport,
    path: &Path,
    kind: &str,
    validator: fn(&Value) -> Vec<String>,
) {
    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Value>(&contents) {
            Ok(value) => {
                match kind {
                    "croissant" => report.croissant_documents += 1,
                    "cdif" => report.cdif_documents += 1,
                    _ => {}
                }
                for message in validator(&value) {
                    report.issues.push(SemanticValidationIssue {
                        path: path.to_path_buf(),
                        kind: kind.to_string(),
                        message,
                    });
                }
            }
            Err(err) => report.issues.push(SemanticValidationIssue {
                path: path.to_path_buf(),
                kind: kind.to_string(),
                message: format!("invalid JSON: {err}"),
            }),
        },
        Err(err) => report.issues.push(SemanticValidationIssue {
            path: path.to_path_buf(),
            kind: kind.to_string(),
            message: format!("cannot read document: {err}"),
        }),
    }
}

fn validate_croissant(value: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    require_context(value, &mut issues);
    require_str(value, "@type", &mut issues);
    require_str(value, "@id", &mut issues);
    require_str(value, "name", &mut issues);
    require_non_empty_array(value, "distribution", &mut issues);
    require_array(value, "recordSet", &mut issues);
    if !matches!(
        value.get("@type").and_then(Value::as_str),
        Some("cr:Dataset" | "sc:Dataset")
    ) {
        issues.push("@type must be cr:Dataset or sc:Dataset".to_string());
    }
    issues
}

fn validate_cdif(value: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    require_context(value, &mut issues);
    require_str(value, "@id", &mut issues);
    require_str(value, "@type", &mut issues);
    require_non_empty_array(value, "cdif:profile", &mut issues);
    require_non_empty_array(value, "dcat:distribution", &mut issues);
    require_array(value, "cdif:dataElement", &mut issues);
    if value.get("@type").and_then(Value::as_str) != Some("dcat:Dataset") {
        issues.push("@type must be dcat:Dataset".to_string());
    }
    issues
}

pub fn validate_openlineage(value: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    require_str(value, "eventType", &mut issues);
    require_str(value, "eventTime", &mut issues);
    require_str(value, "producer", &mut issues);
    require_str(value, "schemaURL", &mut issues);
    if value
        .pointer("/run/runId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push("run.runId is required".to_string());
    }
    if value
        .pointer("/job/namespace")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push("job.namespace is required".to_string());
    }
    if value
        .pointer("/job/name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push("job.name is required".to_string());
    }
    require_array(value, "inputs", &mut issues);
    require_array(value, "outputs", &mut issues);
    issues
}

fn require_str(value: &Value, key: &str, issues: &mut Vec<String>) {
    if value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(format!("{key} is required"));
    }
}

fn require_context(value: &Value, issues: &mut Vec<String>) {
    if !matches!(
        value.get("@context"),
        Some(Value::String(_) | Value::Object(_) | Value::Array(_))
    ) {
        issues.push("@context is required".to_string());
    }
}

fn require_array(value: &Value, key: &str, issues: &mut Vec<String>) {
    if !value.get(key).is_some_and(Value::is_array) {
        issues.push(format!("{key} must be an array"));
    }
}

fn require_non_empty_array(value: &Value, key: &str, issues: &mut Vec<String>) {
    if value
        .get(key)
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        issues.push(format!("{key} must be a non-empty array"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_openlineage_shape() {
        let event = json!({
            "eventType": "COMPLETE",
            "eventTime": "2026-06-13T00:00:00Z",
            "producer": "querygraph",
            "schemaURL": "https://openlineage.io/spec/2-0-2/OpenLineage.json",
            "run": {"runId": "run-1"},
            "job": {"namespace": "querygraph", "name": "demo"},
            "inputs": [],
            "outputs": []
        });
        assert!(validate_openlineage(&event).is_empty());
    }
}
