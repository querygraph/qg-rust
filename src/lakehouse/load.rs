use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::{Field as ArrowField, Schema};
use grust::SailGraphStore;

use crate::cdif::CdifResource;
use crate::dataverse::{DataverseClient, DataverseDataset, DataverseFile};
use crate::sail::safe_sql_name;

use super::infer::{infer_typed_columns, materialize_typed_table_from_chunks};
use super::normalize::{download_if_missing, normalize_tabular_file, sanitize_filename};
use super::project::{
    _record_batch_to_ipc, count_rows, croissant_for_lakehouse, execute_sql, file_sha256, qualified,
    quote_ident,
};
use super::types::*;

pub(crate) fn materialize_catalog_tables(
    reports: &[LakehouseDatasetReport],
    schema: &str,
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    stage_string_rows(
        store,
        runtime,
        "qg_lakehouse_stage_datasets",
        &[
            "dataset_id",
            "persistent_id",
            "title",
            "croissant_path",
            "cdif_path",
        ],
        reports.iter().map(|dataset| {
            vec![
                dataset.id.clone(),
                dataset.persistent_id.clone().unwrap_or_default(),
                dataset.title.clone(),
                dataset.croissant_path.display().to_string(),
                dataset.cdif_path.display().to_string(),
            ]
        }),
    )?;
    replace_table_from_view(
        store,
        runtime,
        schema,
        "lakehouse_datasets",
        "qg_lakehouse_stage_datasets",
    )?;

    stage_string_rows(
        store,
        runtime,
        "qg_lakehouse_stage_files",
        &[
            "dataset_id",
            "file_id",
            "filename",
            "content_type",
            "local_path",
            "size_bytes",
            "sha256",
            "table_name",
            "rows",
            "parse_status",
        ],
        reports.iter().flat_map(|dataset| {
            dataset.files.iter().map(|file| {
                vec![
                    dataset.id.clone(),
                    file.file_id.clone(),
                    file.filename.clone(),
                    file.content_type.clone().unwrap_or_default(),
                    file.local_path.display().to_string(),
                    file.size_bytes.to_string(),
                    file.sha256.clone(),
                    file.table.clone().unwrap_or_default(),
                    file.rows.map(|rows| rows.to_string()).unwrap_or_default(),
                    file.parse_status.clone(),
                ]
            })
        }),
    )?;
    replace_table_from_view(
        store,
        runtime,
        schema,
        "lakehouse_files",
        "qg_lakehouse_stage_files",
    )?;

    stage_string_rows(
        store,
        runtime,
        "qg_lakehouse_stage_columns",
        &[
            "dataset_id",
            "file_id",
            "table_name",
            "source_name",
            "column_name",
            "data_type",
            "nullable",
        ],
        reports.iter().flat_map(|dataset| {
            dataset.files.iter().flat_map(|file| {
                file.columns.iter().map(|column| {
                    vec![
                        dataset.id.clone(),
                        file.file_id.clone(),
                        file.table.clone().unwrap_or_default(),
                        column.source_name.clone(),
                        column.name.clone(),
                        column.data_type.spark_type().to_string(),
                        column.nullable.to_string(),
                    ]
                })
            })
        }),
    )?;
    replace_table_from_view(
        store,
        runtime,
        schema,
        "lakehouse_columns",
        "qg_lakehouse_stage_columns",
    )?;
    Ok(())
}

pub(crate) fn stage_string_rows<I>(
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
    view_name: &str,
    headers: &[&str],
    rows: I,
) -> Result<()>
where
    I: IntoIterator<Item = Vec<String>>,
{
    let rows = rows.into_iter().collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(
        headers
            .iter()
            .map(|header| ArrowField::new(*header, arrow::datatypes::DataType::Utf8, false))
            .collect::<Vec<_>>(),
    ));
    let columns = (0..headers.len())
        .map(|idx| {
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.get(idx).cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )) as Arc<dyn arrow::array::Array>
        })
        .collect::<Vec<_>>();
    let batch = RecordBatch::try_new(schema, columns)?;
    let ipc = _record_batch_to_ipc(batch)?;
    runtime.block_on(store.stage_arrow_ipc_view(view_name, &ipc))?;
    Ok(())
}

fn replace_table_from_view(
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
    schema: &str,
    table: &str,
    view: &str,
) -> Result<()> {
    runtime.block_on(execute_sql(
        store,
        &format!("DROP TABLE IF EXISTS {}", qualified(schema, table)),
    ))?;
    runtime.block_on(execute_sql(
        store,
        &format!(
            "CREATE TABLE {} USING parquet AS SELECT * FROM {}",
            qualified(schema, table),
            quote_ident(view)
        ),
    ))?;
    Ok(())
}

pub(crate) fn load_one_dataset(
    spec: &LakehouseDatasetSpec,
    options: &LakehouseLoadOptions,
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
) -> Result<LakehouseDatasetReport> {
    let dataset_dir = options.root.join("datasets").join(&spec.id);
    let raw_dir = dataset_dir.join("raw");
    let prepared_dir = dataset_dir.join("prepared");
    let semantic_dir = dataset_dir.join("semantic");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&prepared_dir)?;
    fs::create_dir_all(&semantic_dir)?;

    let (dataset, source_files) = resolve_dataset(spec, options.api_token.as_deref(), &raw_dir)?;
    fs::write(
        dataset_dir.join("dataverse-metadata.json"),
        serde_json::to_string_pretty(&dataset)?,
    )?;

    let selected_files = source_files
        .into_iter()
        .take(options.max_files_per_dataset.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    let mut file_reports = Vec::new();
    for file in selected_files {
        file_reports.push(load_one_file(
            spec,
            &dataset,
            &file,
            &raw_dir,
            &prepared_dir,
            &options.schema,
            store,
            runtime,
            options.api_token.as_deref(),
        )?);
    }

    let croissant = croissant_for_lakehouse(&dataset, &file_reports);
    let cdif = CdifResource::from_croissant(
        &croissant,
        dataset.landing_page.clone(),
        format!("sail://{}/{}", options.schema, spec.id),
    );
    let croissant_path = semantic_dir.join("croissant.json");
    let cdif_path = semantic_dir.join("cdif.json");
    fs::write(
        &croissant_path,
        serde_json::to_string_pretty(&croissant.to_json_ld())?,
    )?;
    fs::write(
        &cdif_path,
        serde_json::to_string_pretty(&cdif.to_json_ld())?,
    )?;

    Ok(LakehouseDatasetReport {
        id: spec.id.clone(),
        title: dataset.title,
        persistent_id: spec.persistent_id.clone(),
        files: file_reports,
        croissant_path,
        cdif_path,
    })
}

fn resolve_dataset(
    spec: &LakehouseDatasetSpec,
    api_token: Option<&str>,
    raw_dir: &Path,
) -> Result<(DataverseDataset, Vec<DataverseFile>)> {
    match &spec.source {
        LakehouseSource::Dataverse { base_url } => {
            let persistent_id = spec
                .persistent_id
                .as_deref()
                .context("Dataverse source requires persistent_id")?;
            let mut client = DataverseClient::new(base_url);
            if let Some(api_token) = api_token {
                client = client.with_api_token(api_token);
            }
            let dataset = client
                .get_dataset_by_persistent_id(persistent_id)
                .with_context(|| format!("fetch Dataverse metadata for {persistent_id}"))?;
            let files = dataset.files.clone();
            Ok((dataset, files))
        }
        LakehouseSource::Url { url, filename } => {
            let file = DataverseFile {
                id: spec.id.clone(),
                filename: filename.clone(),
                content_type: Some("text/plain".to_string()),
                download_url: url.clone(),
                description: Some("CODATA/NIST constants ASCII table".to_string()),
            };
            let dataset = DataverseDataset {
                id: spec.id.clone(),
                persistent_id: spec.id.clone(),
                title: spec.title.clone(),
                description: "Trusted CODATA/NIST reference table for constants, units, and measurement semantics.".to_string(),
                authors: vec!["CODATA Task Group on Fundamental Constants".to_string(), "NIST".to_string()],
                subjects: vec!["Physics".to_string(), "Reference Data".to_string()],
                keywords: vec!["CODATA".to_string(), "constants".to_string(), "units".to_string()],
                license: Some("Public domain / NIST SRD 121".to_string()),
                landing_page: "https://physics.nist.gov/constants".to_string(),
                files: vec![file.clone()],
            };
            fs::write(raw_dir.join("source-url.txt"), url)?;
            Ok((dataset, vec![file]))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn load_one_file(
    spec: &LakehouseDatasetSpec,
    _dataset: &DataverseDataset,
    file: &DataverseFile,
    raw_dir: &Path,
    prepared_dir: &Path,
    schema: &str,
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
    api_token: Option<&str>,
) -> Result<LakehouseFileReport> {
    let local_path = raw_dir.join(format!("{}_{}", file.id, sanitize_filename(&file.filename)));
    if let Err(err) = download_if_missing(&file.download_url, &local_path, api_token) {
        return Ok(LakehouseFileReport {
            file_id: file.id.clone(),
            filename: file.filename.clone(),
            content_type: file.content_type.clone(),
            local_path,
            size_bytes: 0,
            sha256: String::new(),
            table: None,
            rows: None,
            columns: Vec::new(),
            parse_status: format!("download_failed: {err:#}"),
        });
    }
    let sha256 = file_sha256(&local_path)?;
    let size_bytes = fs::metadata(&local_path)?.len();

    let normalized = normalize_tabular_file(file, &local_path, prepared_dir)?;
    if let Some(tabular) = normalized {
        eprintln!("  materializing {}", file.filename);
        let table = format!(
            "{}__{}",
            safe_sql_name(&spec.id),
            safe_sql_name(
                Path::new(&tabular.filename)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(&file.id)
            )
        );
        let raw_table = format!("{}_raw", table);
        let columns = infer_typed_columns(&tabular.data_path, tabular.delimiter)?;
        if columns.is_empty() {
            return Ok(asset_report(
                file,
                local_path,
                size_bytes,
                sha256,
                "empty_tabular_file",
            ));
        }
        runtime.block_on(execute_sql(
            store,
            &format!("DROP TABLE IF EXISTS {}", qualified(schema, &table)),
        ))?;
        materialize_typed_table_from_chunks(
            &tabular.data_path,
            tabular.delimiter,
            schema,
            &table,
            &raw_table,
            &columns,
            store,
            runtime,
        )?;
        let rows = runtime.block_on(count_rows(store, schema, &table))?;
        Ok(LakehouseFileReport {
            file_id: file.id.clone(),
            filename: file.filename.clone(),
            content_type: file.content_type.clone(),
            local_path,
            size_bytes,
            sha256,
            table: Some(format!("{}.{}", schema, table)),
            rows: Some(rows),
            columns,
            parse_status: "typed_table_loaded".to_string(),
        })
    } else {
        Ok(asset_report(
            file,
            local_path,
            size_bytes,
            sha256,
            "non_tabular_asset_downloaded",
        ))
    }
}

fn asset_report(
    file: &DataverseFile,
    local_path: PathBuf,
    size_bytes: u64,
    sha256: String,
    status: &str,
) -> LakehouseFileReport {
    LakehouseFileReport {
        file_id: file.id.clone(),
        filename: file.filename.clone(),
        content_type: file.content_type.clone(),
        local_path,
        size_bytes,
        sha256,
        table: None,
        rows: None,
        columns: Vec::new(),
        parse_status: status.to_string(),
    }
}
