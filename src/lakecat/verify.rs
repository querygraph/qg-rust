use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::model::*;

pub(crate) fn validate_view_receipt_evidence(
    views: &[LakeCatViewProjection],
    evidence: &[LakeCatViewReceiptEvidence],
) -> Result<()> {
    if views.is_empty() {
        if evidence.is_empty() {
            return Ok(());
        }
        bail!("LakeCat QueryGraph import contract carries view receipt evidence without views");
    }
    if evidence.len() != views.len() {
        bail!(
            "LakeCat QueryGraph import contract lists {} view receipt evidence record(s) for {} view(s)",
            evidence.len(),
            views.len()
        );
    }
    for view in views {
        let Some(record) = evidence
            .iter()
            .find(|record| record.stable_id == view.stable_id)
        else {
            bail!(
                "LakeCat QueryGraph import contract is missing view receipt evidence for {}",
                view.stable_id
            );
        };
        if record.view_version != view.view_version {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} has version {}, expected {}",
                view.stable_id,
                record.view_version,
                view.view_version
            );
        }
        if record.receipt_hash.is_empty() {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} is missing a receipt hash",
                view.stable_id
            );
        }
        if record.receipt_chain_hash.is_empty() {
            bail!(
                "LakeCat QueryGraph import contract view receipt evidence for {} is missing a receipt-chain hash",
                view.stable_id
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_duplicate_free_stable_ids<'a>(
    label: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            bail!("{label} must be duplicate-free by stable id: {value}");
        }
    }
    Ok(())
}

pub(crate) fn table_only_artifact(artifact: &LakeCatTableArtifactHashes) -> Value {
    serde_json::json!({
        "stable-id": artifact.stable_id,
        "croissant-hash": artifact.croissant_hash,
        "cdif-hash": artifact.cdif_hash,
        "osi-hash": artifact.osi_hash,
        "odrl-hash": artifact.odrl_hash,
    })
}

pub(crate) fn table_only_projection(table: &LakeCatTableProjection) -> Value {
    serde_json::json!({
        "ident": table.ident,
        "stable-id": table.stable_id,
        "location": table.location,
        "metadata-location": table.metadata_location,
        "version": table.version,
        "format-version": table.format_version,
        "croissant": table.croissant,
        "cdif": table.cdif,
        "osi": table.osi,
        "odrl": table.odrl,
    })
}

pub(crate) fn assert_hash(label: &str, expected: &str, value: &Value) -> Result<()> {
    let actual = content_hash_json(value)?;
    if expected != actual {
        bail!("{label} hash mismatch: manifest={expected} actual={actual}");
    }
    Ok(())
}

pub(crate) fn content_hash_json(value: &Value) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("failed to encode JSON for LakeCat hash")?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

pub(crate) fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToString::to_string)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
