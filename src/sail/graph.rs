use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use arrow::array::{Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field as ArrowField, Schema};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use grust::prelude::*;
use grust::SailGraphStore;
use serde_json::{Value, json};

use crate::dataverse::DataverseDataset;
use crate::osi::OsiDocument;

use super::{LiveSailViewReport, safe_sql_name};

pub(super) fn json_view_sql(view_name: &str, path: &Path) -> String {
    format!(
        "CREATE OR REPLACE TEMP VIEW {view_name} USING json OPTIONS (path '{}');",
        path.display()
    )
}

pub(super) fn dataset_metadata_jsonl(dataset: &DataverseDataset) -> Result<String> {
    Ok(format!(
        "{}\n",
        serde_json::to_string(&json!({
            "id": dataset.id,
            "persistent_id": dataset.persistent_id,
            "title": dataset.title,
            "description": dataset.description,
            "authors": dataset.authors,
            "subjects": dataset.subjects,
            "keywords": dataset.keywords,
            "license": dataset.license,
            "landing_page": dataset.landing_page,
        }))?
    ))
}

pub(super) fn dataset_files_jsonl(dataset: &DataverseDataset) -> Result<String> {
    let mut lines = String::new();
    for file in &dataset.files {
        lines.push_str(&serde_json::to_string(&json!({
            "dataset_persistent_id": dataset.persistent_id,
            "file_id": file.id,
            "file_name": file.filename,
            "content_type": file.content_type,
            "download_url": file.download_url,
            "description": file.description,
        }))?);
        lines.push('\n');
    }
    Ok(lines)
}

pub(super) async fn stage_dataverse_views(
    store: &SailGraphStore,
    datasets: &[DataverseDataset],
) -> Result<Vec<LiveSailViewReport>> {
    let mut reports = Vec::new();
    for dataset in datasets {
        let table_name = safe_sql_name(&format!("dataverse_{}", dataset.id));
        let metadata_view = format!("{table_name}_metadata");
        let files_view = format!("{table_name}_files");
        store
            .stage_arrow_ipc_view(&metadata_view, &metadata_ipc(dataset)?)
            .await?;
        store
            .stage_arrow_ipc_view(&files_view, &files_ipc(dataset)?)
            .await?;
        reports.push(LiveSailViewReport {
            view_name: metadata_view.clone(),
            rows: count_view_rows(store, &metadata_view).await?,
        });
        reports.push(LiveSailViewReport {
            view_name: files_view.clone(),
            rows: count_view_rows(store, &files_view).await?,
        });
    }
    Ok(reports)
}

async fn count_view_rows(store: &SailGraphStore, view_name: &str) -> Result<i64> {
    let chunks = store
        .query_arrow_ipc(&format!("SELECT COUNT(*) AS row_count FROM {view_name}"))
        .await?;
    for chunk in chunks {
        let mut reader = StreamReader::try_new(Cursor::new(chunk), None)?;
        for batch in &mut reader {
            let batch = batch?;
            if batch.num_columns() == 0 || batch.num_rows() == 0 {
                continue;
            }
            let column = batch.column(0);
            if let Some(values) = column.as_any().downcast_ref::<Int64Array>() {
                return Ok(values.value(0));
            }
        }
    }
    Ok(0)
}

fn metadata_ipc(dataset: &DataverseDataset) -> Result<Vec<u8>> {
    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("id", DataType::Utf8, false),
        ArrowField::new("persistent_id", DataType::Utf8, false),
        ArrowField::new("title", DataType::Utf8, false),
        ArrowField::new("description", DataType::Utf8, false),
        ArrowField::new("authors", DataType::Utf8, false),
        ArrowField::new("subjects", DataType::Utf8, false),
        ArrowField::new("keywords", DataType::Utf8, false),
        ArrowField::new("license", DataType::Utf8, true),
        ArrowField::new("landing_page", DataType::Utf8, false),
    ]));
    record_batch_to_ipc(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![dataset.id.as_str()])),
            Arc::new(StringArray::from(vec![dataset.persistent_id.as_str()])),
            Arc::new(StringArray::from(vec![dataset.title.as_str()])),
            Arc::new(StringArray::from(vec![dataset.description.as_str()])),
            Arc::new(StringArray::from(vec![dataset.authors.join(", ")])),
            Arc::new(StringArray::from(vec![dataset.subjects.join(", ")])),
            Arc::new(StringArray::from(vec![dataset.keywords.join(", ")])),
            Arc::new(StringArray::from(vec![dataset.license.as_deref()])),
            Arc::new(StringArray::from(vec![dataset.landing_page.as_str()])),
        ],
    )?)
}

fn files_ipc(dataset: &DataverseDataset) -> Result<Vec<u8>> {
    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("dataset_persistent_id", DataType::Utf8, false),
        ArrowField::new("file_id", DataType::Utf8, false),
        ArrowField::new("file_name", DataType::Utf8, false),
        ArrowField::new("content_type", DataType::Utf8, true),
        ArrowField::new("download_url", DataType::Utf8, false),
        ArrowField::new("description", DataType::Utf8, true),
    ]));
    let persistent_ids = vec![dataset.persistent_id.clone(); dataset.files.len()];
    record_batch_to_ipc(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(persistent_ids)),
            Arc::new(StringArray::from(
                dataset
                    .files
                    .iter()
                    .map(|file| file.id.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                dataset
                    .files
                    .iter()
                    .map(|file| file.filename.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                dataset
                    .files
                    .iter()
                    .map(|file| file.content_type.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                dataset
                    .files
                    .iter()
                    .map(|file| file.download_url.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                dataset
                    .files
                    .iter()
                    .map(|file| file.description.as_deref())
                    .collect::<Vec<_>>(),
            )),
        ],
    )?)
}

fn record_batch_to_ipc(batch: RecordBatch) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut bytes, &batch.schema())?;
        writer.write(&batch)?;
        writer.finish()?;
    }
    Ok(bytes)
}

pub(crate) fn dataverse_semantic_graph(
    datasets: &[DataverseDataset],
    bundle: &Value,
    osi: Option<&OsiDocument>,
) -> Graph {
    let mut builder = Graph::builder();
    let bundle_id = bundle["identity"]["id"]
        .as_str()
        .unwrap_or("querygraph:bundle:unknown");
    let _ = builder
        .node("NavigatorBundle", bundle_id)
        .prop("bundle_type", "querygraph:AiNavigatorSemanticBundle")
        .prop(
            "generated_at",
            bundle["generatedAt"].as_str().unwrap_or("unknown"),
        )
        .finish();

    for dataset in datasets {
        let dataset_node = format!("dataverse:dataset:{}", dataset.persistent_id);
        let _ = builder
            .node("DataverseDataset", dataset_node.as_str())
            .prop("persistent_id", dataset.persistent_id.as_str())
            .prop("title", dataset.title.as_str())
            .prop("landing_page", dataset.landing_page.as_str())
            .finish();
        let _ = builder
            .edge("described_by", dataset_node.as_str(), bundle_id)
            .finish();

        for file in &dataset.files {
            let file_node = format!("dataverse:file:{}", file.id);
            let _ = builder
                .node("DataverseFile", file_node.as_str())
                .prop("file_id", file.id.as_str())
                .prop("filename", file.filename.as_str())
                .prop("download_url", file.download_url.as_str())
                .finish();
            let _ = builder
                .edge("has_file", dataset_node.as_str(), file_node.as_str())
                .finish();
        }

        for keyword in &dataset.keywords {
            let keyword_node = format!("keyword:{}", safe_sql_name(keyword));
            let _ = builder
                .node("OntologyTerm", keyword_node.as_str())
                .prop("label", keyword.as_str())
                .finish();
            let _ = builder
                .edge("tagged_with", dataset_node.as_str(), keyword_node.as_str())
                .finish();
        }
    }

    if let Some(elements) = bundle["layers"]["cdif"]["cdif:dataElement"].as_array() {
        for element in elements {
            if let Some(id) = element["@id"].as_str() {
                let _ = builder
                    .node("CdifDataElement", id)
                    .prop(
                        "title",
                        element["dct:title"].as_str().unwrap_or("data element"),
                    )
                    .prop(
                        "semantic_type",
                        element["cdif:semanticType"].as_str().unwrap_or(""),
                    )
                    .finish();
                let _ = builder.edge("projects_element", bundle_id, id).finish();
            }
        }
    }

    if let Some(osi) = osi {
        let model = &osi.semantic_model;
        let model_node = format!("osi:model:{}", model.name);
        let _ = builder
            .node("OsiModel", model_node.as_str())
            .prop("name", model.name.as_str())
            .prop("version", osi.version.as_str())
            .prop("description", model.description.as_deref().unwrap_or(""))
            .finish();
        let _ = builder
            .edge("wrapped_by", model_node.as_str(), bundle_id)
            .finish();

        for dataset in &model.datasets {
            let dataset_node = format!("osi:dataset:{}:{}", model.name, dataset.name);
            let _ = builder
                .node("OsiDataset", dataset_node.as_str())
                .prop("name", dataset.name.as_str())
                .prop("source", dataset.source.as_str())
                .prop("description", dataset.description.as_deref().unwrap_or(""))
                .finish();
            let _ = builder
                .edge("has_dataset", model_node.as_str(), dataset_node.as_str())
                .finish();
            for field in &dataset.fields {
                let field_node =
                    format!("osi:field:{}:{}:{}", model.name, dataset.name, field.name);
                let _ = builder
                    .node("OsiField", field_node.as_str())
                    .prop("name", field.name.as_str())
                    .prop(
                        "semantic_type",
                        field.semantic_type.as_deref().unwrap_or(""),
                    )
                    .prop("description", field.description.as_deref().unwrap_or(""))
                    .finish();
                let _ = builder
                    .edge("has_field", dataset_node.as_str(), field_node.as_str())
                    .finish();
            }
        }

        for metric in &model.metrics {
            let metric_node = format!("osi:metric:{}:{}", model.name, metric.name);
            let expression = metric
                .expression
                .dialects
                .first()
                .map(|expr| format!("{}: {}", expr.dialect, expr.expression))
                .unwrap_or_default();
            let _ = builder
                .node("OsiMetric", metric_node.as_str())
                .prop("name", metric.name.as_str())
                .prop("description", metric.description.as_deref().unwrap_or(""))
                .prop("expression", expression)
                .finish();
            let _ = builder
                .edge("has_metric", model_node.as_str(), metric_node.as_str())
                .finish();
        }

        for term in &model.ontology_terms {
            let _ = builder
                .node("OntologyTerm", term.id.as_str())
                .prop("label", term.label.as_str())
                .prop("source", term.source.as_deref().unwrap_or(""))
                .finish();
            let _ = builder
                .edge("uses_ontology_term", model_node.as_str(), term.id.as_str())
                .finish();
        }
    }

    builder.build()
}
