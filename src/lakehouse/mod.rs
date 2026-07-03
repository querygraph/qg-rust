use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use grust::{SailConfig, SailGraphStore};
use serde_json::{Value, json};

mod infer;
mod load;
mod normalize;
mod project;
mod types;

pub use self::types::*;

use self::load::{load_one_dataset, materialize_catalog_tables};
use self::project::{
    collect_string_column, count_qualified_rows, execute_sql, query_sql, quote_ident,
};

pub const DEFAULT_SCHEMA: &str = "qg_lakehouse";

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
