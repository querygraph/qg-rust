use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use arrow::array::{Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{Field as ArrowField, Schema};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use calamine::{Data, Reader, open_workbook_auto};
use grust::{SailConfig, SailGraphStore};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::cdif::CdifResource;
use crate::croissant::{CroissantDataset, Field, FileObject, RecordSet};
use crate::dataverse::{DataverseClient, DataverseDataset, DataverseFile};
use crate::sail::safe_sql_name;

pub const DEFAULT_SCHEMA: &str = "qg_lakehouse";
const ROWS_PER_SAIL_CHUNK: usize = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseDatasetSpec {
    pub id: String,
    pub persistent_id: Option<String>,
    pub title: String,
    pub source: LakehouseSource,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum LakehouseSource {
    Dataverse { base_url: String },
    Url { url: String, filename: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseLoadOptions {
    pub root: PathBuf,
    pub schema: String,
    pub sail_endpoint: String,
    pub max_files_per_dataset: Option<usize>,
    pub api_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseReport {
    pub root: PathBuf,
    pub schema: String,
    pub endpoint: String,
    pub datasets: Vec<LakehouseDatasetReport>,
    pub catalog_tables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseVerifyReport {
    pub endpoint: String,
    pub schema: String,
    pub typed_tables: usize,
    pub manifest_rows: i64,
    pub sail_rows: i64,
    pub tables: Vec<LakehouseVerifyTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseVerifyTable {
    pub table: String,
    pub manifest_rows: i64,
    pub sail_rows: i64,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseDatasetReport {
    pub id: String,
    pub title: String,
    pub persistent_id: Option<String>,
    pub files: Vec<LakehouseFileReport>,
    pub croissant_path: PathBuf,
    pub cdif_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseFileReport {
    pub file_id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub local_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub table: Option<String>,
    pub rows: Option<i64>,
    pub columns: Vec<TypedColumn>,
    pub parse_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypedColumn {
    pub source_name: String,
    pub name: String,
    pub data_type: LakehouseDataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LakehouseDataType {
    Boolean,
    Int64,
    Float64,
    Date,
    Timestamp,
    String,
}

impl LakehouseDataType {
    fn spark_type(self) -> &'static str {
        match self {
            Self::Boolean => "BOOLEAN",
            Self::Int64 => "BIGINT",
            Self::Float64 => "DOUBLE",
            Self::Date => "DATE",
            Self::Timestamp => "TIMESTAMP",
            Self::String => "STRING",
        }
    }

    fn croissant_type(self) -> &'static str {
        match self {
            Self::Boolean => "sc:Boolean",
            Self::Int64 => "sc:Integer",
            Self::Float64 => "sc:Float",
            Self::Date => "sc:Date",
            Self::Timestamp => "sc:DateTime",
            Self::String => "sc:Text",
        }
    }
}

pub fn default_dataset_specs() -> Vec<LakehouseDatasetSpec> {
    vec![
        dataverse_spec(
            "government_finance",
            "doi:10.7910/DVN/LMS8NT",
            "The Government Finance Database",
            "finance",
        ),
        dataverse_spec(
            "roadway_lidar",
            "doi:10.7910/DVN/1VT6FZ",
            "Roadway vulnerability LiDAR DTM",
            "geospatial",
        ),
        dataverse_spec(
            "access_2018_energy",
            "doi:10.7910/DVN/AHFINM",
            "Access to Clean Cooking Energy and Electricity: Survey of States in India 2018",
            "energy",
        ),
        dataverse_spec(
            "dockless_transportation",
            "doi:10.7910/DVN/B2LJSB",
            "Dockless transportation hotspots and mode shift",
            "transportation",
        ),
        dataverse_spec(
            "haalsi_baseline",
            "doi:10.7910/DVN/F5YHML",
            "HAALSI Baseline Survey",
            "health",
        ),
        dataverse_spec(
            "global_party_survey",
            "doi:10.7910/DVN/WMGTNS",
            "Global Party Survey, 2019",
            "social_science",
        ),
        dataverse_spec(
            "pedestrian_injury_ct",
            "doi:10.7910/DVN/TXIKF9",
            "Pedestrian injury severity in Connecticut",
            "transportation",
        ),
        dataverse_spec(
            "energy_insecurity_covid",
            "doi:10.7910/DVN/OMJWNB",
            "Energy insecurity among low-income households during COVID-19",
            "energy",
        ),
        dataverse_spec(
            "climate_health_pathways",
            "doi:10.7910/DVN/DHDNIC",
            "Exploring Climate and Health Pathways in the INSPIRE Network",
            "climate_health",
        ),
        LakehouseDatasetSpec {
            id: "codata_constants_2022".to_string(),
            persistent_id: None,
            title: "CODATA/NIST 2022 Fundamental Physical Constants".to_string(),
            category: "reference".to_string(),
            source: LakehouseSource::Url {
                url: "https://physics.nist.gov/cuu/Constants/Table/allascii.txt".to_string(),
                filename: "codata_constants_2022_allascii.txt".to_string(),
            },
        },
    ]
}

fn dataverse_spec(
    id: &str,
    persistent_id: &str,
    title: &str,
    category: &str,
) -> LakehouseDatasetSpec {
    LakehouseDatasetSpec {
        id: id.to_string(),
        persistent_id: Some(persistent_id.to_string()),
        title: title.to_string(),
        category: category.to_string(),
        source: LakehouseSource::Dataverse {
            base_url: "https://dataverse.harvard.edu".to_string(),
        },
    }
}

pub fn load_default_lakehouse(options: LakehouseLoadOptions) -> Result<LakehouseReport> {
    load_lakehouse(&default_dataset_specs(), options)
}

pub fn load_lakehouse(
    specs: &[LakehouseDatasetSpec],
    options: LakehouseLoadOptions,
) -> Result<LakehouseReport> {
    fs::create_dir_all(&options.root)?;
    let manifest_dir = options.root.join("manifest");
    fs::create_dir_all(&manifest_dir)?;
    fs::write(
        manifest_dir.join("datasets.json"),
        serde_json::to_string_pretty(specs)?,
    )?;

    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(SailGraphStore::connect(SailConfig {
        endpoint: options.sail_endpoint.clone(),
        user_id: "querygraph-lakehouse".to_string(),
        session_id: "querygraph-lakehouse-loader".to_string(),
        batch_size: 1000,
    }))?;
    runtime.block_on(execute_sql(
        &store,
        &format!(
            "CREATE DATABASE IF NOT EXISTS {}",
            quote_ident(&options.schema)
        ),
    ))?;

    let mut reports = Vec::new();
    for spec in specs {
        eprintln!("loading dataset {} ({})", spec.id, spec.title);
        reports.push(load_one_dataset(spec, &options, &store, &runtime)?);
    }
    materialize_catalog_tables(&reports, &options.schema, &store, &runtime)?;

    let catalog_tables = collect_string_column(&runtime.block_on(query_sql(
        &store,
        &format!("SHOW TABLES IN {}", quote_ident(&options.schema)),
    ))?);

    let report = LakehouseReport {
        root: options.root.clone(),
        schema: options.schema,
        endpoint: options.sail_endpoint,
        datasets: reports,
        catalog_tables,
    };
    fs::write(
        manifest_dir.join("load-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

fn materialize_catalog_tables(
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

fn stage_string_rows<I>(
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

fn load_one_dataset(
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

#[derive(Debug, Clone)]
struct NormalizedTabularFile {
    data_path: PathBuf,
    filename: String,
    delimiter: u8,
}

fn normalize_tabular_file(
    file: &DataverseFile,
    local_path: &Path,
    prepared_dir: &Path,
) -> Result<Option<NormalizedTabularFile>> {
    let name = file.filename.to_ascii_lowercase();
    let content_type = file
        .content_type
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if spec_is_codata(file) {
        let out = prepared_dir.join("codata_constants_2022").join("data.csv");
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        normalize_codata_constants(local_path, &out)?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out,
            filename: "codata_constants_2022.csv".to_string(),
            delimiter: b',',
        }));
    }
    if name.ends_with(".csv") || content_type.contains("text/csv") {
        let out = prepared_copy(local_path, prepared_dir, &file.filename, "csv")?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out.join(format!("data.{}", "csv")),
            filename: file.filename.clone(),
            delimiter: b',',
        }));
    }
    if name.ends_with(".tab")
        || name.ends_with(".tsv")
        || content_type.contains("tab-separated")
        || content_type.contains("text/tab")
    {
        let out = prepared_copy(local_path, prepared_dir, &file.filename, "tsv")?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out.join(format!("data.{}", "tsv")),
            filename: file.filename.clone(),
            delimiter: b'\t',
        }));
    }
    if name.ends_with(".xlsx") {
        let out_dir = prepared_dir.join(safe_sql_name(
            Path::new(&file.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file.id),
        ));
        fs::create_dir_all(&out_dir)?;
        let out = out_dir.join(format!(
            "{}.csv",
            safe_sql_name(
                Path::new(&file.filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&file.id)
            )
        ));
        xlsx_first_sheet_to_csv(local_path, &out)?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out,
            filename: format!("{}.csv", file.filename),
            delimiter: b',',
        }));
    }
    Ok(None)
}

fn prepared_copy(
    source: &Path,
    prepared_dir: &Path,
    filename: &str,
    extension: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(prepared_dir)?;
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("table");
    let out_dir = prepared_dir.join(safe_sql_name(stem));
    fs::create_dir_all(&out_dir)?;
    let out = out_dir.join(format!("data.{}", extension));
    if !out.exists() || fs::metadata(&out)?.len() == 0 {
        fs::copy(source, &out)?;
    }
    Ok(out_dir)
}

fn spec_is_codata(file: &DataverseFile) -> bool {
    file.id == "codata_constants_2022" || file.filename.contains("codata_constants")
}

fn download_if_missing(url: &str, path: &Path, api_token: Option<&str>) -> Result<()> {
    if path.exists() && fs::metadata(path)?.len() > 0 {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("download");
    let client = Client::builder()
        .timeout(Duration::from_secs(1800))
        .build()?;
    let mut request = client.get(url);
    if let Some(api_token) = api_token {
        request = request.header("X-Dataverse-key", api_token);
    }
    let mut response = request
        .send()
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?;
    let mut out = File::create(&tmp)?;
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
    }
    out.flush()?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn infer_typed_columns(path: &Path, delimiter: u8) -> Result<Vec<TypedColumn>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_path(path)?;
    let headers = reader.headers()?.clone();
    let mut states = headers
        .iter()
        .enumerate()
        .map(|(idx, name)| ColumnInference::new(name, idx))
        .collect::<Vec<_>>();
    for result in reader.records().take(5000) {
        let record = result?;
        for (idx, state) in states.iter_mut().enumerate() {
            state.observe(record.get(idx).unwrap_or(""));
        }
    }
    Ok(states
        .into_iter()
        .map(|state| state.finish())
        .collect::<Vec<_>>())
}

#[derive(Debug, Clone)]
struct ColumnInference {
    source_name: String,
    name: String,
    nullable: bool,
    saw_value: bool,
    boolean_ok: bool,
    int_ok: bool,
    float_ok: bool,
    date_ok: bool,
    timestamp_ok: bool,
}

impl ColumnInference {
    fn new(source_name: &str, index: usize) -> Self {
        let mut name = safe_sql_name(source_name);
        if name.is_empty() {
            name = format!("column_{index}");
        }
        Self {
            source_name: source_name.to_string(),
            name,
            nullable: false,
            saw_value: false,
            boolean_ok: true,
            int_ok: true,
            float_ok: true,
            date_ok: true,
            timestamp_ok: true,
        }
    }

    fn observe(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() || matches!(value, "." | "NA" | "N/A" | "null" | "NULL") {
            self.nullable = true;
            return;
        }
        self.saw_value = true;
        self.boolean_ok &= is_bool(value);
        self.int_ok &= parse_int(value).is_some();
        self.float_ok &= parse_float(value).is_some();
        self.date_ok &= looks_like_date(value);
        self.timestamp_ok &= looks_like_timestamp(value);
    }

    fn finish(self) -> TypedColumn {
        let data_type = if !self.saw_value {
            LakehouseDataType::String
        } else if self.boolean_ok {
            LakehouseDataType::Boolean
        } else if self.int_ok {
            LakehouseDataType::Int64
        } else if self.float_ok {
            LakehouseDataType::Float64
        } else if self.date_ok {
            LakehouseDataType::Date
        } else if self.timestamp_ok {
            LakehouseDataType::Timestamp
        } else {
            LakehouseDataType::String
        };
        TypedColumn {
            source_name: self.source_name,
            name: self.name,
            data_type,
            nullable: self.nullable,
        }
    }
}

fn parse_int(value: &str) -> Option<i64> {
    value.replace(',', "").parse().ok()
}

fn parse_float(value: &str) -> Option<f64> {
    value.replace(',', "").parse().ok()
}

fn is_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "0" | "1"
    )
}

fn looks_like_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, ch)| idx == 4 || idx == 7 || ch.is_ascii_digit())
}

fn looks_like_timestamp(value: &str) -> bool {
    value.contains('T') && value.len() >= 19
}

fn create_typed_table_sql(
    schema: &str,
    table: &str,
    stage_view: &str,
    columns: &[TypedColumn],
) -> String {
    let select = columns
        .iter()
        .map(|column| {
            format!(
                "TRY_CAST({} AS {}) AS {}",
                quote_ident(&column.source_name),
                column.data_type.spark_type(),
                quote_ident(&column.name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE TABLE {} USING parquet AS SELECT {} FROM {}",
        qualified(schema, table),
        select,
        quote_ident(stage_view)
    )
}

fn insert_typed_table_sql(
    schema: &str,
    table: &str,
    stage_view: &str,
    columns: &[TypedColumn],
) -> String {
    let select = columns
        .iter()
        .map(|column| {
            format!(
                "TRY_CAST({} AS {}) AS {}",
                quote_ident(&column.source_name),
                column.data_type.spark_type(),
                quote_ident(&column.name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {} SELECT {} FROM {}",
        qualified(schema, table),
        select,
        quote_ident(stage_view)
    )
}

fn materialize_typed_table_from_chunks(
    path: &Path,
    delimiter: u8,
    schema: &str,
    table: &str,
    stage_view: &str,
    columns: &[TypedColumn],
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_path(path)?;
    let headers = reader
        .headers()?
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut chunk = Vec::new();
    let mut created = false;
    for record in reader.records() {
        let record = record?;
        chunk.push(
            (0..headers.len())
                .map(|idx| record.get(idx).unwrap_or("").to_string())
                .collect::<Vec<_>>(),
        );
        if chunk.len() >= ROWS_PER_SAIL_CHUNK {
            stage_and_write_chunk(
                std::mem::take(&mut chunk),
                &headers,
                schema,
                table,
                stage_view,
                columns,
                store,
                runtime,
                &mut created,
            )?;
        }
    }
    stage_and_write_chunk(
        chunk,
        &headers,
        schema,
        table,
        stage_view,
        columns,
        store,
        runtime,
        &mut created,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_and_write_chunk(
    rows: Vec<Vec<String>>,
    headers: &[String],
    schema: &str,
    table: &str,
    stage_view: &str,
    columns: &[TypedColumn],
    store: &SailGraphStore,
    runtime: &tokio::runtime::Runtime,
    created: &mut bool,
) -> Result<()> {
    if rows.is_empty() && *created {
        return Ok(());
    }
    let header_refs = headers.iter().map(String::as_str).collect::<Vec<_>>();
    stage_string_rows(store, runtime, stage_view, &header_refs, rows)?;
    if *created {
        runtime.block_on(execute_sql(
            store,
            &insert_typed_table_sql(schema, table, stage_view, columns),
        ))?;
    } else {
        runtime.block_on(execute_sql(
            store,
            &create_typed_table_sql(schema, table, stage_view, columns),
        ))?;
        *created = true;
    }
    Ok(())
}

async fn execute_sql(store: &SailGraphStore, sql: &str) -> Result<()> {
    store
        .query_arrow_ipc(sql)
        .await
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("Sail SQL failed: {sql}: {err}"))
}

async fn query_sql(store: &SailGraphStore, sql: &str) -> Result<Vec<Vec<u8>>> {
    store
        .query_arrow_ipc(sql)
        .await
        .map_err(|err| anyhow::anyhow!("Sail SQL failed: {sql}: {err}"))
}

async fn count_rows(store: &SailGraphStore, schema: &str, table: &str) -> Result<i64> {
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

fn collect_string_column(chunks: &[Vec<u8>]) -> Vec<String> {
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

fn croissant_for_lakehouse(
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

fn xlsx_first_sheet_to_csv(path: &Path, out: &Path) -> Result<()> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .context("workbook has no sheets")?;
    let range = workbook.worksheet_range(&sheet)?;
    let mut writer = csv::Writer::from_path(out)?;
    for row in range.rows() {
        writer.write_record(row.iter().map(cell_to_string))?;
    }
    writer.flush()?;
    Ok(())
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("{value:?}"),
    }
}

fn normalize_codata_constants(path: &Path, out: &Path) -> Result<()> {
    let reader = BufReader::new(File::open(path)?);
    let mut writer = csv::Writer::from_path(out)?;
    writer.write_record(["quantity", "value", "uncertainty", "unit"])?;
    let mut in_table = false;
    for line in reader.lines() {
        let line = line?;
        if line.starts_with("----") {
            in_table = true;
            continue;
        }
        if !in_table || line.trim().is_empty() {
            continue;
        }
        if line.len() < 90 {
            continue;
        }
        let quantity = line.get(0..60).unwrap_or("").trim();
        let value = line.get(60..85).unwrap_or("").trim();
        let uncertainty = line.get(85..110).unwrap_or("").trim();
        let unit = line.get(110..).unwrap_or("").trim();
        if !quantity.is_empty() {
            writer.write_record([quantity, value, uncertainty, unit])?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
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

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| if ch == '/' || ch == ':' { '_' } else { ch })
        .collect()
}

fn quote_ident(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn qualified(schema: &str, table: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(table))
}

pub fn report_summary(report: &LakehouseReport) -> Value {
    let mut datasets = BTreeMap::new();
    for dataset in &report.datasets {
        datasets.insert(
            dataset.id.clone(),
            json!({
                "title": dataset.title,
                "files": dataset.files.len(),
                "typedTables": dataset.files.iter().filter(|file| file.table.is_some()).count(),
                "rows": dataset.files.iter().filter_map(|file| file.rows).sum::<i64>(),
            }),
        );
    }
    json!({
        "schema": report.schema,
        "endpoint": report.endpoint,
        "datasets": datasets,
        "catalogTables": report.catalog_tables,
    })
}

pub fn verify_lakehouse_report(
    report_path: impl AsRef<Path>,
    endpoint: impl Into<String>,
) -> Result<LakehouseVerifyReport> {
    let report: LakehouseReport = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let endpoint = endpoint.into();
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(SailGraphStore::connect(SailConfig {
        endpoint: endpoint.clone(),
        user_id: "querygraph-lakehouse-verify".to_string(),
        session_id: "querygraph-lakehouse-loader".to_string(),
        batch_size: 1000,
    }))?;
    let mut tables = Vec::new();
    for file in report
        .datasets
        .iter()
        .flat_map(|dataset| dataset.files.iter())
    {
        if let (Some(table), Some(manifest_rows)) = (&file.table, file.rows) {
            let sail_rows = runtime.block_on(count_qualified_rows(&store, table))?;
            tables.push(LakehouseVerifyTable {
                table: table.clone(),
                manifest_rows,
                sail_rows,
                ok: manifest_rows == sail_rows,
            });
        }
    }
    let manifest_rows = tables.iter().map(|table| table.manifest_rows).sum();
    let sail_rows = tables.iter().map(|table| table.sail_rows).sum();
    Ok(LakehouseVerifyReport {
        endpoint,
        schema: report.schema,
        typed_tables: tables.len(),
        manifest_rows,
        sail_rows,
        tables,
    })
}

async fn count_qualified_rows(store: &SailGraphStore, table: &str) -> Result<i64> {
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

fn _qualified_name(table: &str) -> String {
    if let Some((schema, table)) = table.split_once('.') {
        qualified(schema, table)
    } else {
        quote_ident(table)
    }
}

#[allow(dead_code)]
fn _record_batch_to_ipc(batch: RecordBatch) -> Result<Vec<u8>> {
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
