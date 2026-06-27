use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use arrow::array::{Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::Schema;
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use grust::SailGraphStore;
use sha2::{Digest, Sha256};

use crate::croissant::{CroissantDataset, Field, FileObject, RecordSet};
use crate::dataverse::DataverseDataset;
use crate::sail::safe_sql_name;

use super::types::LakehouseFileReport;

pub(crate) async fn execute_sql(store: &SailGraphStore, sql: &str) -> Result<()> {
    store
        .query_arrow_ipc(sql)
        .await
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("Sail SQL failed: {sql}: {err}"))
}

pub(crate) async fn query_sql(store: &SailGraphStore, sql: &str) -> Result<Vec<Vec<u8>>> {
    store
        .query_arrow_ipc(sql)
        .await
        .map_err(|err| anyhow::anyhow!("Sail SQL failed: {sql}: {err}"))
}

pub(crate) async fn count_rows(store: &SailGraphStore, schema: &str, table: &str) -> Result<i64> {
    let chunks = query_sql(
        store,
        &format!(
            "SELECT COUNT(*) AS row_count FROM {}",
            qualified(schema, table)
        ),
    )
    .await?;
    first_i64(&chunks).context("row count missing")
}

pub(crate) async fn count_qualified_rows(store: &SailGraphStore, table: &str) -> Result<i64> {
    let (qualified_name, fallback_name) = if let Some((schema, table)) = table.split_once('.') {
        (qualified(schema, table), Some(quote_ident(table)))
    } else {
        (quote_ident(table), None)
    };
    let sql = format!("SELECT COUNT(*) AS row_count FROM {}", qualified_name);
    match query_sql(store, &sql).await {
        Ok(chunks) => first_i64(&chunks).context("row count missing"),
        Err(err) => {
            if let Some(fallback_name) = fallback_name {
                let chunks = query_sql(
                    store,
                    &format!("SELECT COUNT(*) AS row_count FROM {}", fallback_name),
                )
                .await?;
                first_i64(&chunks).context("row count missing")
            } else {
                Err(err)
            }
        }
    }
}

pub(crate) fn collect_string_column(chunks: &[Vec<u8>]) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in chunks {
        if let Ok(mut reader) = StreamReader::try_new(std::io::Cursor::new(chunk), None) {
            for batch in (&mut reader).flatten() {
                for idx in 0..batch.num_columns() {
                    if let Some(values) = batch.column(idx).as_any().downcast_ref::<StringArray>() {
                        for row in 0..values.len() {
                            if !values.is_null(row) {
                                out.push(values.value(row).to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn first_i64(chunks: &[Vec<u8>]) -> Option<i64> {
    for chunk in chunks {
        let mut reader = StreamReader::try_new(std::io::Cursor::new(chunk), None).ok()?;
        for batch in (&mut reader).flatten() {
            if batch.num_columns() == 0 || batch.num_rows() == 0 {
                continue;
            }
            if let Some(values) = batch.column(0).as_any().downcast_ref::<Int64Array>() {
                return Some(values.value(0));
            }
        }
    }
    None
}

pub(crate) fn croissant_for_lakehouse(
    dataset: &DataverseDataset,
    reports: &[LakehouseFileReport],
) -> CroissantDataset {
    let dataset_id = format!("{}/#lakehouse", dataset.landing_page.trim_end_matches('/'));
    let files = reports
        .iter()
        .map(|report| FileObject {
            id: format!("{dataset_id}/file/{}", report.file_id),
            name: report.filename.clone(),
            content_url: report.local_path.display().to_string(),
            encoding_format: report
                .content_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string()),
        })
        .collect::<Vec<_>>();
    let record_sets = reports
        .iter()
        .filter_map(|report| {
            report.table.as_ref().map(|table| RecordSet {
                id: format!("{dataset_id}/recordset/{}", safe_sql_name(table)),
                name: table.clone(),
                fields: report
                    .columns
                    .iter()
                    .map(|column| {
                        Field::new(
                            column.name.clone(),
                            column.data_type.croissant_type(),
                            format!("Lakehouse column derived from {}", column.source_name),
                        )
                        .semantic_type(semantic_type_for_column(&column.name))
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    CroissantDataset {
        id: dataset_id,
        name: dataset.title.clone(),
        description: dataset.description.clone(),
        license: dataset
            .license
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        creators: dataset.authors.clone(),
        files,
        record_sets,
        keywords: dataset
            .keywords
            .iter()
            .chain(dataset.subjects.iter())
            .cloned()
            .collect(),
    }
}

fn semantic_type_for_column(name: &str) -> String {
    if name.contains("date") || name.contains("year") {
        "https://schema.org/temporalCoverage".to_string()
    } else if name.contains("lat")
        || name.contains("lon")
        || name.contains("state")
        || name.contains("country")
    {
        "https://schema.org/spatialCoverage".to_string()
    } else if name.contains("amount") || name.contains("cost") || name.contains("revenue") {
        "https://schema.org/MonetaryAmount".to_string()
    } else {
        "https://schema.org/variableMeasured".to_string()
    }
}

pub(crate) fn file_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn quote_ident(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

pub(crate) fn qualified(schema: &str, table: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(table))
}

fn _qualified_name(table: &str) -> String {
    if let Some((schema, table)) = table.split_once('.') {
        qualified(schema, table)
    } else {
        quote_ident(table)
    }
}

#[allow(dead_code)]
pub(crate) fn _record_batch_to_ipc(batch: RecordBatch) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut writer = StreamWriter::try_new(&mut bytes, &batch.schema())?;
    writer.write(&batch)?;
    writer.finish()?;
    Ok(bytes)
}

#[allow(dead_code)]
fn _string_batch(schema: Arc<Schema>, values: Vec<Vec<String>>) -> Result<RecordBatch> {
    if values.len() != schema.fields().len() {
        bail!("value column count does not match schema");
    }
    let arrays = values
        .into_iter()
        .map(|column| Arc::new(StringArray::from(column)) as Arc<dyn arrow::array::Array>)
        .collect::<Vec<_>>();
    Ok(RecordBatch::try_new(schema, arrays)?)
}
