use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use grust::prelude::*;
use grust::{SailConfig, SailGraphStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dataverse::DataverseDataset;
use crate::osi::OsiDocument;

mod graph;

use self::graph::{
    dataset_files_jsonl, dataset_metadata_jsonl, dataverse_semantic_graph, json_view_sql,
    stage_dataverse_views,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SailDatasetLoad {
    pub dataset_id: String,
    pub table_name: String,
    pub metadata_path: PathBuf,
    pub files_path: PathBuf,
    pub create_metadata_view_sql: String,
    pub create_files_view_sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SailLoadReport {
    pub root: PathBuf,
    pub loads: Vec<SailDatasetLoad>,
    pub bootstrap_sql: Vec<String>,
    pub graph: Option<LiveSailGraphReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveSailGraphReport {
    pub endpoint: String,
    pub views: Vec<LiveSailViewReport>,
    pub nodes: usize,
    pub edges: usize,
    pub loaded_nodes: usize,
    pub loaded_edges: usize,
    pub verified_node_id: Option<String>,
    pub verified_node_label: Option<String>,
    /// Distinct node labels in the loaded semantic graph, from a Grust 0.11
    /// "Crab" `CALL db.labels()` Cypher read.
    pub cypher_labels: Vec<String>,
    /// `DataverseDataset` nodes seen by a `MATCH (d:DataverseDataset) RETURN
    /// count(d)` Cypher read against the loaded graph.
    pub cypher_dataset_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveSailViewReport {
    pub view_name: String,
    pub rows: i64,
}

#[derive(Debug, Clone)]
pub struct LocalSailLakehouse {
    root: PathBuf,
}

impl LocalSailLakehouse {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn stage_dataverse_datasets(
        &self,
        datasets: &[DataverseDataset],
    ) -> Result<SailLoadReport> {
        fs::create_dir_all(&self.root)?;
        let mut loads = Vec::new();
        let mut bootstrap_sql = Vec::new();

        for dataset in datasets {
            let table_name = safe_sql_name(&format!("dataverse_{}", dataset.id));
            let dataset_dir = self.root.join(&table_name);
            fs::create_dir_all(&dataset_dir)?;

            let metadata_path = dataset_dir.join("metadata.jsonl");
            let files_path = dataset_dir.join("files.jsonl");
            fs::write(&metadata_path, dataset_metadata_jsonl(dataset)?)?;
            fs::write(&files_path, dataset_files_jsonl(dataset)?)?;

            let metadata_view = format!("{table_name}_metadata");
            let files_view = format!("{table_name}_files");
            let create_metadata_view_sql = json_view_sql(&metadata_view, &metadata_path);
            let create_files_view_sql = json_view_sql(&files_view, &files_path);
            bootstrap_sql.push(create_metadata_view_sql.clone());
            bootstrap_sql.push(create_files_view_sql.clone());

            loads.push(SailDatasetLoad {
                dataset_id: dataset.persistent_id.clone(),
                table_name,
                metadata_path,
                files_path,
                create_metadata_view_sql,
                create_files_view_sql,
            });
        }

        Ok(SailLoadReport {
            root: self.root.clone(),
            loads,
            bootstrap_sql,
            graph: None,
        })
    }
}

impl SailLoadReport {
    pub async fn load_semantic_graph_into_sail(
        mut self,
        endpoint: impl Into<String>,
        datasets: &[DataverseDataset],
        bundle: &Value,
        osi: Option<&OsiDocument>,
    ) -> Result<Self> {
        let endpoint = endpoint.into();
        let graph = dataverse_semantic_graph(datasets, bundle, osi);
        let (cypher_labels, cypher_dataset_count) = summarize_semantic_graph(&graph)?;
        let store = SailGraphStore::connect(SailConfig {
            endpoint: endpoint.clone(),
            user_id: "querygraph".to_string(),
            session_id: "querygraph-dataverse-e2e".to_string(),
            batch_size: 500,
        })
        .await?;
        let views = stage_dataverse_views(&store, datasets).await?;
        store.bootstrap().await?;
        let loaded = store.put_graph(&graph).await?;
        let verified = if let Some(dataset) = datasets.first() {
            let id = NodeId::from(format!("dataverse:dataset:{}", dataset.persistent_id));
            store.get_node(&id).await?
        } else {
            None
        };
        self.graph = Some(LiveSailGraphReport {
            endpoint,
            views,
            nodes: graph.nodes.len(),
            edges: graph.edges.len(),
            loaded_nodes: loaded.nodes,
            loaded_edges: loaded.edges,
            verified_node_id: verified.as_ref().map(|node| node.id.as_str().to_string()),
            verified_node_label: verified
                .as_ref()
                .map(|node| node.label.as_str().to_string()),
            cypher_labels,
            cypher_dataset_count,
        });
        Ok(self)
    }
}

/// Summarize a QueryGraph semantic graph with Grust 0.11 "Crab" Cypher: the
/// distinct node labels (`CALL db.labels()`) and the Dataverse-dataset count
/// (`MATCH (d:DataverseDataset) RETURN count(d)`). The Memory reference executor
/// produces the same answers a live Sail backend pushes down, so this runs over
/// the in-memory graph before it is loaded.
fn summarize_semantic_graph(graph: &Graph) -> Result<(Vec<String>, i64)> {
    let mut labels = crate::cypher::labels(graph)?;
    labels.sort();
    let dataset_count = crate::cypher::label_count(graph, "DataverseDataset")?;
    Ok((labels, dataset_count))
}

pub fn safe_sql_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert_str(0, "qg_");
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataverse::sample_datasets;
    use serde_json::json;

    #[test]
    fn stages_dataverse_jsonl_for_sail() {
        let root =
            std::env::temp_dir().join(format!("querygraph-sail-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let report = LocalSailLakehouse::new(&root)
            .stage_dataverse_datasets(&sample_datasets())
            .expect("datasets should stage");

        assert_eq!(report.loads.len(), 2);
        assert!(report.loads[0].metadata_path.exists());
        assert!(report.bootstrap_sql[0].contains("CREATE OR REPLACE TEMP VIEW"));
        assert!(report.graph.is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn builds_dataverse_semantic_graph() {
        let bundle = json!({
            "identity": {"id": "did:example:bundle"},
            "generatedAt": "2026-06-14T00:00:00Z",
            "layers": {"cdif": {"cdif:dataElement": [{
                "@id": "field:one",
                "dct:title": "field one",
                "cdif:semanticType": "https://schema.org/name"
            }]}}
        });
        let osi = OsiDocument::for_dataverse(&sample_datasets());
        let graph = dataverse_semantic_graph(&sample_datasets(), &bundle, Some(&osi));

        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.label.as_str() == "DataverseDataset")
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.label.as_str() == "OsiMetric")
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.label.as_str() == "described_by")
        );
    }

    #[test]
    fn summarizes_semantic_graph_with_cypher() {
        let bundle = json!({
            "identity": {"id": "did:example:bundle"},
            "generatedAt": "2026-06-14T00:00:00Z",
            "layers": {"cdif": {"cdif:dataElement": []}}
        });
        let datasets = sample_datasets();
        let graph = dataverse_semantic_graph(&datasets, &bundle, None);
        let (labels, dataset_count) = summarize_semantic_graph(&graph).expect("cypher summary");

        assert!(labels.contains(&"DataverseDataset".to_string()));
        assert!(labels.windows(2).all(|pair| pair[0] <= pair[1]), "sorted");
        assert_eq!(dataset_count, datasets.len() as i64);
    }
}
