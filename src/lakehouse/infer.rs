use std::path::Path;

use anyhow::Result;
use grust::SailGraphStore;

use crate::sail::safe_sql_name;

use super::load::stage_string_rows;
use super::project::{execute_sql, qualified, quote_ident};
use super::types::{LakehouseDataType, TypedColumn};

const ROWS_PER_SAIL_CHUNK: usize = 30_000;

pub(crate) fn infer_typed_columns(path: &Path, delimiter: u8) -> Result<Vec<TypedColumn>> {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_typed_table_from_chunks(
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
