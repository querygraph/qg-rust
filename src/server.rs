//! First slice of the documented `/v1` HTTP API
//! (`docs/sail-typesec-grust-implementation.md` §"API Surface").
//!
//! Makes the governed semantic layer reachable over a network: build
//! four-layer Navigator bundles, run the QGLake governance story, and verify
//! TypeDID envelopes (including qg-python's Ed25519 envelopes). Verification
//! results and policy denials are first-class 200 responses carrying
//! receipts — an invalid signature is a finding, not a server error.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    agent::{PyEnvelopeVerification, PyTypeDidEnvelope},
    navigator::{AiNavigator, NavigatorInput},
    osi::{OsiDocument, OsiSemanticModel},
    qglake::run_qglake_story,
};

/// In-memory semantic-model registry, keyed by model name.
type ModelRegistry = Arc<RwLock<BTreeMap<String, OsiDocument>>>;

pub fn router() -> Router {
    let registry: ModelRegistry = Arc::default();
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/navigator/bundle", post(navigator_bundle))
        .route("/v1/qglake/story", get(qglake_story))
        .route("/v1/audit/verify-envelope", post(verify_envelope))
        .route("/v1/models", get(list_models))
        .route("/v1/models/{name}", get(get_model))
        .route("/v1/models/import/osi", post(import_osi))
        .route("/v1/models/import/croissant", post(import_croissant))
        .route("/v1/search", get(search_models))
        .route("/.well-known/agent-card.json", get(agent_card))
        .with_state(registry)
}

async fn agent_card(headers: axum::http::HeaderMap) -> Json<Value> {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost:8080");
    Json(crate::a2a::agent_card(&format!("http://{host}")))
}

pub async fn serve(port: u16) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    eprintln!("qg-server listening on http://0.0.0.0:{port}/v1");
    axum::serve(listener, router()).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "querygraph",
        "api": "v1",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn navigator_bundle(Json(input): Json<NavigatorInput>) -> Json<Value> {
    Json(AiNavigator.build(input).bundle)
}

async fn qglake_story() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // The story signs a dozen envelopes; keep it off the async worker.
    let report = tokio::task::spawn_blocking(run_qglake_story)
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    serde_json::to_value(report)
        .map(Json)
        .map_err(internal_error)
}

async fn verify_envelope(Json(envelope): Json<PyTypeDidEnvelope>) -> Json<PyEnvelopeVerification> {
    Json(envelope.verify())
}

async fn list_models(State(registry): State<ModelRegistry>) -> Json<Value> {
    let models = registry.read().expect("registry lock");
    Json(json!({
        "models": models
            .values()
            .map(|document| json!({
                "name": document.semantic_model.name,
                "description": document.semantic_model.description,
                "datasets": document.semantic_model.datasets.len(),
                "metrics": document.semantic_model.metrics.len(),
            }))
            .collect::<Vec<_>>(),
    }))
}

async fn get_model(
    State(registry): State<ModelRegistry>,
    Path(name): Path<String>,
) -> Result<Json<OsiDocument>, (StatusCode, Json<Value>)> {
    registry
        .read()
        .expect("registry lock")
        .get(&name)
        .cloned()
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("no semantic model named {name:?}")})),
            )
        })
}

async fn import_osi(
    State(registry): State<ModelRegistry>,
    Json(document): Json<OsiDocument>,
) -> Json<Value> {
    Json(register_model(&registry, document))
}

async fn import_croissant(
    State(registry): State<ModelRegistry>,
    Json(croissant): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let document =
        OsiDocument::from_croissant_json(&croissant, "qg_lakehouse").map_err(|error| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": error.to_string()})),
            )
        })?;
    Ok(Json(register_model(&registry, document)))
}

fn register_model(registry: &ModelRegistry, document: OsiDocument) -> Value {
    let model = &document.semantic_model;
    let summary = json!({
        "imported": model.name,
        "datasets": model.datasets.len(),
        "metrics": model.metrics.len(),
        "ontologyTerms": model.ontology_terms.len(),
    });
    registry
        .write()
        .expect("registry lock")
        .insert(model.name.clone(), document);
    summary
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    model: Option<String>,
}

async fn search_models(
    State(registry): State<ModelRegistry>,
    Query(params): Query<SearchParams>,
) -> Json<Value> {
    let needle = params.q.to_lowercase();
    let models = registry.read().expect("registry lock");
    let matches: Vec<Value> = models
        .values()
        .filter(|document| {
            params
                .model
                .as_deref()
                .is_none_or(|name| name == document.semantic_model.name)
        })
        .flat_map(|document| search_model(&document.semantic_model, &needle))
        .collect();
    Json(json!({"query": params.q, "matches": matches}))
}

/// Case-insensitive containment search over names, descriptions, ai_context,
/// semantic types, and ontology labels — the same surface qg-python's
/// `find_by_synonym` covers, extended to descriptions.
fn search_model(model: &OsiSemanticModel, needle: &str) -> Vec<Value> {
    let hit = |text: &Option<String>| {
        text.as_deref()
            .is_some_and(|value| value.to_lowercase().contains(needle))
    };
    let name_hit = |name: &str| name.to_lowercase().contains(needle);
    let mut matches = Vec::new();
    for dataset in &model.datasets {
        if name_hit(&dataset.name) || hit(&dataset.description) || hit(&dataset.ai_context) {
            matches.push(json!({
                "model": model.name, "kind": "dataset", "name": dataset.name,
            }));
        }
        for field in &dataset.fields {
            if name_hit(&field.name) || hit(&field.description) || hit(&field.semantic_type) {
                matches.push(json!({
                    "model": model.name, "kind": "field",
                    "name": field.name, "dataset": dataset.name,
                }));
            }
        }
    }
    for metric in &model.metrics {
        if name_hit(&metric.name) || hit(&metric.description) || hit(&metric.ai_context) {
            matches.push(json!({
                "model": model.name, "kind": "metric", "name": metric.name,
            }));
        }
    }
    for term in &model.ontology_terms {
        if name_hit(&term.label) || name_hit(&term.id) {
            matches.push(json!({
                "model": model.name, "kind": "ontologyTerm",
                "name": term.label, "id": term.id,
            }));
        }
    }
    matches
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": error.to_string()})),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn call(request: Request<Body>) -> (StatusCode, Value) {
        let response = router().oneshot(request).await.expect("router responds");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body collects")
            .to_bytes();
        (status, serde_json::from_slice(&bytes).expect("JSON body"))
    }

    fn post_json(uri: &str, body: Value) -> Request<Body> {
        Request::post(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request builds")
    }

    #[tokio::test]
    async fn health_reports_service_and_version() {
        let (status, body) = call(Request::get("/v1/health").body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["service"], "querygraph");
        assert_eq!(body["api"], "v1");
    }

    #[tokio::test]
    async fn navigator_bundle_builds_four_layers() {
        let (status, body) = call(post_json(
            "/v1/navigator/bundle",
            json!({
                "dataset_name": "Hazard vocabulary",
                "description": "Controlled vocabulary with multilingual technical terms",
                "landing_page": "https://querygraph.ai/datasets/hazards",
                "data_url": "https://querygraph.ai/datasets/hazards.csv",
                "creator": "QueryGraph",
                "agent_name": "AI Navigator",
            }),
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        let layers = &body["layers"];
        for layer in ["semanticCroissant", "cdif", "did", "odrl"] {
            assert!(!layers[layer].is_null(), "missing layer {layer}");
        }
        assert_eq!(body["@type"], "querygraph:AiNavigatorSemanticBundle");
    }

    #[tokio::test]
    async fn qglake_story_serves_the_evidence_chain() {
        let (status, body) = call(
            Request::get("/v1/qglake/story")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["specialists"].as_array().map(Vec::len), Some(6));
        assert_eq!(
            body["did_attestation"]["signature_type"],
            "Ed25519Signature2020"
        );
    }

    #[tokio::test]
    async fn model_import_search_and_fetch_roundtrip() {
        let router = router();

        // Import a Croissant JSON-LD document; it projects to an OSI model.
        let croissant = json!({
            "name": "Energy Burden",
            "description": "Demo energy fields",
            "distribution": [{"name": "energy.parquet"}],
            "recordSet": [{
                "name": "observations",
                "field": [{
                    "name": "monthly_cost",
                    "description": "Monthly household energy cost",
                    "sameAs": "https://querygraph.ai/ontology/monthlyEnergyCost",
                }],
            }],
        });
        let response = router
            .clone()
            .oneshot(post_json("/v1/models/import/croissant", croissant))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["imported"], "energy_burden_semantic_model");

        // The registry lists it, search finds the field, and the full
        // document fetches by name.
        let response = router
            .clone()
            .oneshot(Request::get("/v1/models").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["models"][0]["name"], "energy_burden_semantic_model");

        let response = router
            .clone()
            .oneshot(
                Request::get("/v1/search?q=monthly")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let kinds: Vec<&str> = body["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["kind"].as_str().unwrap())
            .collect();
        assert!(kinds.contains(&"field"));

        let response = router
            .clone()
            .oneshot(
                Request::get("/v1/models/energy_burden_semantic_model")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::get("/v1/models/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn verify_envelope_accepts_python_fixture_and_flags_tampering() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/py_envelope.json"))
                .expect("fixture parses");

        let (status, body) = call(post_json("/v1/audit/verify-envelope", fixture.clone())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["signature_valid"], true);

        let mut tampered = fixture;
        tampered["resource"] = json!("compartment:other");
        let (status, body) = call(post_json("/v1/audit/verify-envelope", tampered)).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "a bad signature is a finding, not an error"
        );
        assert_eq!(body["signature_valid"], false);
    }
}
