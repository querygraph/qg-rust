use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::croissant::{CroissantDataset, Field, FileObject, RecordSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataverseDataset {
    pub id: String,
    pub persistent_id: String,
    pub title: String,
    pub description: String,
    pub authors: Vec<String>,
    pub subjects: Vec<String>,
    pub keywords: Vec<String>,
    pub license: Option<String>,
    pub landing_page: String,
    pub files: Vec<DataverseFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataverseFile {
    pub id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub download_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DataverseClient {
    base_url: String,
    api_token: Option<String>,
}

impl DataverseClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_token: None,
        }
    }

    pub fn with_api_token(mut self, api_token: impl Into<String>) -> Self {
        self.api_token = Some(api_token.into());
        self
    }

    pub fn search_datasets(
        &self,
        query: &str,
        per_page: usize,
    ) -> Result<Vec<DataverseDataset>, reqwest::Error> {
        let client = reqwest::blocking::Client::new();
        let mut request = client.get(format!("{}/api/search", self.base_url)).query(&[
            ("q", query),
            ("type", "dataset"),
            ("per_page", &per_page.to_string()),
        ]);
        if let Some(token) = &self.api_token {
            request = request.header("X-Dataverse-key", token);
        }
        let response: Value = request.send()?.error_for_status()?.json()?;
        let items = response["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let mut datasets = Vec::new();
        for item in items {
            if let Some(persistent_id) = item["global_id"]
                .as_str()
                .or_else(|| item["persistentUrl"].as_str())
            {
                if let Ok(dataset) = self.get_dataset_by_persistent_id(persistent_id) {
                    datasets.push(dataset);
                }
            }
        }
        Ok(datasets)
    }

    pub fn get_dataset_by_persistent_id(
        &self,
        persistent_id: &str,
    ) -> Result<DataverseDataset, reqwest::Error> {
        let client = reqwest::blocking::Client::new();
        let mut request = client
            .get(format!("{}/api/datasets/:persistentId", self.base_url))
            .query(&[("persistentId", persistent_id)]);
        if let Some(token) = &self.api_token {
            request = request.header("X-Dataverse-key", token);
        }
        let response: Value = request.send()?.error_for_status()?.json()?;
        Ok(parse_dataset(&self.base_url, &response))
    }
}

pub fn parse_dataset(base_url: &str, response: &Value) -> DataverseDataset {
    let data = &response["data"];
    let latest = &data["latestVersion"];
    let citation_fields = latest["metadataBlocks"]["citation"]["fields"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let persistent_id = data["persistentId"]
        .as_str()
        .or_else(|| latest["datasetPersistentId"].as_str())
        .unwrap_or("dataverse:unknown")
        .to_string();
    let id = data["id"]
        .as_i64()
        .map(|id| id.to_string())
        .unwrap_or_else(|| persistent_id.clone());
    let title = field_string(&citation_fields, "title").unwrap_or_else(|| persistent_id.clone());
    let description =
        field_compound_strings(&citation_fields, "dsDescription", "dsDescriptionValue")
            .into_iter()
            .next()
            .unwrap_or_else(|| "Dataverse dataset".to_string());
    let authors = field_compound_strings(&citation_fields, "author", "authorName");
    let subjects = field_controlled_values(&citation_fields, "subject");
    let keywords = field_compound_strings(&citation_fields, "keyword", "keywordValue");
    let license = latest["license"]["name"]
        .as_str()
        .or_else(|| latest["termsOfUse"].as_str())
        .map(ToString::to_string);
    let landing_page =
        if persistent_id.starts_with("http://") || persistent_id.starts_with("https://") {
            persistent_id.clone()
        } else {
            format!(
                "{}/dataset.xhtml?persistentId={persistent_id}",
                base_url.trim_end_matches('/')
            )
        };
    let files = latest["files"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|file| parse_file(base_url, file))
        .collect();

    DataverseDataset {
        id,
        persistent_id,
        title,
        description,
        authors,
        subjects,
        keywords,
        license,
        landing_page,
        files,
    }
}

impl DataverseDataset {
    pub fn to_croissant(&self) -> CroissantDataset {
        let dataset_id = format!("{}/#dataset", self.landing_page.trim_end_matches('/'));
        let files = self
            .files
            .iter()
            .map(|file| FileObject {
                id: format!("{dataset_id}/file/{}", file.id),
                name: file.filename.clone(),
                content_url: file.download_url.clone(),
                encoding_format: file
                    .content_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            })
            .collect::<Vec<_>>();

        let record_sets = self
            .files
            .iter()
            .map(|file| {
                let stem = file
                    .filename
                    .rsplit_once('.')
                    .map(|(stem, _)| stem)
                    .unwrap_or(&file.filename);
                RecordSet {
                    id: format!("{dataset_id}/recordset/{}", safe_fragment(stem)),
                    name: stem.to_string(),
                    fields: vec![
                        Field::new(
                            "dataset_persistent_id",
                            "sc:Text",
                            "Dataverse persistent ID",
                        )
                        .semantic_type("https://schema.org/identifier"),
                        Field::new("file_id", "sc:Text", "Dataverse file ID")
                            .semantic_type("https://schema.org/identifier"),
                        Field::new("file_name", "sc:Text", "Dataverse file name")
                            .semantic_type("https://schema.org/name"),
                        Field::new("download_url", "sc:URL", "Dataverse file download URL")
                            .semantic_type("https://schema.org/contentUrl"),
                    ],
                }
            })
            .collect::<Vec<_>>();

        CroissantDataset {
            id: dataset_id,
            name: self.title.clone(),
            description: self.description.clone(),
            license: self.license.clone().unwrap_or_else(|| {
                "https://dataverse.org/best-practices/harvard-dataverse-general-terms-use"
                    .to_string()
            }),
            creators: if self.authors.is_empty() {
                vec!["Dataverse".to_string()]
            } else {
                self.authors.clone()
            },
            files,
            record_sets,
            keywords: self
                .keywords
                .iter()
                .chain(self.subjects.iter())
                .cloned()
                .collect(),
        }
    }
}

pub fn sample_datasets() -> Vec<DataverseDataset> {
    vec![
        DataverseDataset {
            id: "1001".to_string(),
            persistent_id: "doi:10.7910/DVN/QGDEMO1".to_string(),
            title: "Bay Area building energy observations".to_string(),
            description:
                "Sample Dataverse-shaped energy observations for QueryGraph integration testing."
                    .to_string(),
            authors: vec!["QueryGraph Demo Lab".to_string()],
            subjects: vec![
                "Engineering".to_string(),
                "Computer and Information Science".to_string(),
            ],
            keywords: vec![
                "energy".to_string(),
                "buildings".to_string(),
                "sensors".to_string(),
            ],
            license: Some("CC0 1.0".to_string()),
            landing_page:
                "https://demo.dataverse.org/dataset.xhtml?persistentId=doi:10.7910/DVN/QGDEMO1"
                    .to_string(),
            files: vec![DataverseFile {
                id: "501".to_string(),
                filename: "building_energy.csv".to_string(),
                content_type: Some("text/csv".to_string()),
                download_url: "https://demo.dataverse.org/api/access/datafile/501".to_string(),
                description: Some("Hourly energy observations.".to_string()),
            }],
        },
        DataverseDataset {
            id: "1002".to_string(),
            persistent_id: "doi:10.7910/DVN/QGDEMO2".to_string(),
            title: "Enterprise data access survey".to_string(),
            description: "Sample Dataverse-shaped survey metadata for governed agent access."
                .to_string(),
            authors: vec!["QueryGraph Governance Lab".to_string()],
            subjects: vec!["Social Sciences".to_string()],
            keywords: vec![
                "access control".to_string(),
                "governance".to_string(),
                "survey".to_string(),
            ],
            license: Some("CC BY 4.0".to_string()),
            landing_page:
                "https://demo.dataverse.org/dataset.xhtml?persistentId=doi:10.7910/DVN/QGDEMO2"
                    .to_string(),
            files: vec![DataverseFile {
                id: "502".to_string(),
                filename: "access_survey.tab".to_string(),
                content_type: Some("text/tab-separated-values".to_string()),
                download_url: "https://demo.dataverse.org/api/access/datafile/502".to_string(),
                description: Some("Survey responses.".to_string()),
            }],
        },
    ]
}

fn parse_file(base_url: &str, file: &Value) -> DataverseFile {
    let data_file = &file["dataFile"];
    let id = data_file["id"]
        .as_i64()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let filename = data_file["filename"]
        .as_str()
        .unwrap_or("dataverse-file")
        .to_string();
    DataverseFile {
        download_url: format!(
            "{}/api/access/datafile/{id}",
            base_url.trim_end_matches('/')
        ),
        id,
        filename,
        content_type: data_file["contentType"].as_str().map(ToString::to_string),
        description: file["description"].as_str().map(ToString::to_string),
    }
}

fn field_string(fields: &[Value], type_name: &str) -> Option<String> {
    fields
        .iter()
        .find(|field| field["typeName"] == type_name)
        .and_then(|field| field["value"].as_str())
        .map(ToString::to_string)
}

fn field_controlled_values(fields: &[Value], type_name: &str) -> Vec<String> {
    fields
        .iter()
        .find(|field| field["typeName"] == type_name)
        .and_then(|field| field["value"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(ToString::to_string))
        .collect()
}

fn field_compound_strings(fields: &[Value], type_name: &str, child_name: &str) -> Vec<String> {
    fields
        .iter()
        .find(|field| field["typeName"] == type_name)
        .and_then(|field| field["value"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|compound| {
            compound[child_name]["value"]
                .as_str()
                .map(ToString::to_string)
        })
        .collect()
}

fn safe_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_dataverse_native_dataset_response() {
        let response = json!({
            "data": {
                "id": 42,
                "persistentId": "doi:10.7910/DVN/ABC123",
                "latestVersion": {
                    "license": {"name": "CC0 1.0"},
                    "metadataBlocks": {
                        "citation": {
                            "fields": [
                                {"typeName": "title", "value": "Example dataset"},
                                {"typeName": "subject", "value": ["Computer and Information Science"]},
                                {"typeName": "author", "value": [{"authorName": {"value": "Ada Lovelace"}}]},
                                {"typeName": "keyword", "value": [{"keywordValue": {"value": "metadata"}}]},
                                {"typeName": "dsDescription", "value": [{"dsDescriptionValue": {"value": "Useful data."}}]}
                            ]
                        }
                    },
                    "files": [{
                        "description": "Rows",
                        "dataFile": {
                            "id": 99,
                            "filename": "rows.csv",
                            "contentType": "text/csv"
                        }
                    }]
                }
            }
        });

        let dataset = parse_dataset("https://demo.dataverse.org", &response);
        assert_eq!(dataset.title, "Example dataset");
        assert_eq!(dataset.authors, vec!["Ada Lovelace"]);
        assert_eq!(
            dataset.files[0].download_url,
            "https://demo.dataverse.org/api/access/datafile/99"
        );
    }

    #[test]
    fn projects_dataverse_dataset_to_croissant() {
        let dataset = sample_datasets().remove(0);
        let croissant = dataset.to_croissant();
        assert_eq!(croissant.files.len(), 1);
        assert_eq!(
            croissant.record_sets[0].fields[0].name,
            "dataset_persistent_id"
        );
    }
}
