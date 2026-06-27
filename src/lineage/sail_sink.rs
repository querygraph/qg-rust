use arrow::array::StringArray;
use arrow::datatypes::{DataType, Field as ArrowField, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use chrono::Utc;
use grust::{SailConfig, SailGraphStore};
use serde_json::Value;
use std::sync::Arc;

use super::{LineageAttestation, OpenLineageEmission, OpenLineageRunEvent};

pub fn emit_openlineage_sail(
    endpoint: impl Into<String>,
    schema: impl AsRef<str>,
    event: &OpenLineageRunEvent,
    attestation: &LineageAttestation,
) -> anyhow::Result<OpenLineageEmission> {
    let endpoint = endpoint.into();
    let schema = schema.as_ref();
    let event_hash = event.event_hash();
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(SailGraphStore::connect(SailConfig {
        endpoint: endpoint.clone(),
        user_id: "querygraph-openlineage".to_string(),
        session_id: "querygraph-openlineage-audit".to_string(),
        batch_size: 1000,
    }))?;
    runtime.block_on(execute_sql(
        &store,
        &format!("CREATE DATABASE IF NOT EXISTS {}", quote_ident(schema)),
    ))?;

    let event_json = serde_json::to_string(event)?;
    append_rows(
        &runtime,
        &store,
        schema,
        "openlineage_events",
        "qg_openlineage_events_stage",
        &[
            "event_hash",
            "event_type",
            "event_time",
            "producer",
            "schema_url",
            "run_id",
            "job_namespace",
            "job_name",
            "event_json",
        ],
        vec![vec![
            event_hash.clone(),
            event.event_type.clone(),
            event.event_time.to_rfc3339(),
            event.producer.clone(),
            event.schema_url.clone(),
            event
                .run
                .get("runId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            event
                .job
                .get("namespace")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            event
                .job
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            event_json,
        ]],
    )?;

    append_rows(
        &runtime,
        &store,
        schema,
        "openlineage_attestations",
        "qg_openlineage_attestations_stage",
        &[
            "event_hash",
            "issuer",
            "subject",
            "merkle_root",
            "signature_type",
            "verification_method",
            "signature",
            "signed_payload_sha256",
            "created_at",
            "attestation_json",
        ],
        vec![vec![
            attestation.event_hash.clone(),
            attestation.issuer.clone(),
            attestation.subject.clone(),
            attestation.merkle_root.clone(),
            attestation.signature_type.clone(),
            attestation.verification_method.clone(),
            attestation.signature.clone(),
            attestation.signed_payload_sha256.clone(),
            attestation.created_at.to_rfc3339(),
            serde_json::to_string(attestation)?,
        ]],
    )?;

    Ok(OpenLineageEmission {
        target: format!("sail://{endpoint}/{schema}.openlineage_events"),
        event_hash,
        status: "sail_appended".to_string(),
        http_status: None,
        path: None,
        emitted_at: Utc::now(),
    })
}

fn append_rows(
    runtime: &tokio::runtime::Runtime,
    store: &SailGraphStore,
    schema: &str,
    table: &str,
    view: &str,
    headers: &[&str],
    rows: Vec<Vec<String>>,
) -> anyhow::Result<()> {
    let ipc = string_rows_ipc(headers, rows)?;
    runtime.block_on(store.stage_arrow_ipc_view(view, &ipc))?;
    let table_name = qualified(schema, table);
    let view_name = quote_ident(view);
    if runtime
        .block_on(execute_sql(
            store,
            &format!("SELECT COUNT(*) AS row_count FROM {table_name}"),
        ))
        .is_ok()
    {
        runtime.block_on(execute_sql(
            store,
            &format!("INSERT INTO {table_name} SELECT * FROM {view_name}"),
        ))?;
    } else {
        runtime.block_on(execute_sql(
            store,
            &format!("CREATE TABLE {table_name} USING parquet AS SELECT * FROM {view_name}"),
        ))?;
    }
    Ok(())
}

fn string_rows_ipc(headers: &[&str], rows: Vec<Vec<String>>) -> anyhow::Result<Vec<u8>> {
    let schema = Arc::new(Schema::new(
        headers
            .iter()
            .map(|header| ArrowField::new(*header, DataType::Utf8, false))
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
    let mut bytes = Vec::new();
    let mut writer = StreamWriter::try_new(&mut bytes, &batch.schema())?;
    writer.write(&batch)?;
    writer.finish()?;
    drop(writer);
    Ok(bytes)
}

async fn execute_sql(store: &SailGraphStore, sql: &str) -> anyhow::Result<()> {
    store
        .query_arrow_ipc(sql)
        .await
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("Sail SQL failed: {sql}: {err}"))
}

fn qualified(schema: &str, table: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(table))
}

fn quote_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}
