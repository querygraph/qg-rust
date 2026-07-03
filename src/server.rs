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
    body::Body,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    agent::{PyEnvelopeVerification, PyTypeDidEnvelope, TypeDidEnvelope},
    lineage::run_id_for,
    navigator::{AiNavigator, NavigatorInput},
    osi::{OsiDocument, OsiSemanticModel},
    qglake::run_qglake_story,
};

/// In-memory semantic-model registry, keyed by model name.
type ModelRegistry = Arc<RwLock<BTreeMap<String, OsiDocument>>>;

pub fn router() -> Router {
    router_with_options(false)
}

/// With `require_auth`, mutating/answering routes demand a signed TypeDID
/// envelope in the `x-qg-envelope` header: `action == "invoke"`, `resource`
/// bound to the request path, `payload.bodySha256` bound to the request
/// body, and an Ed25519 signature verifiable against the envelope's did:key
/// verification method. Failures return 401 with a receipt.
pub fn router_with_options(require_auth: bool) -> Router {
    let registry: ModelRegistry = Arc::default();
    let mut governed = Router::new()
        .route("/v1/models/import/osi", post(import_osi))
        .route("/v1/models/import/croissant", post(import_croissant))
        .route("/v1/answer", post(answer));
    if require_auth {
        governed = governed.route_layer(middleware::from_fn(envelope_auth));
    }
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/navigator/bundle", post(navigator_bundle))
        .route("/v1/qglake/story", get(qglake_story))
        .route("/v1/audit/verify-envelope", post(verify_envelope))
        .route("/v1/models", get(list_models))
        .route("/v1/models/{name}", get(get_model))
        .route("/v1/search", get(search_models))
        .route("/.well-known/agent-card.json", get(agent_card))
        .merge(governed)
        .with_state(registry)
}

async fn envelope_auth(
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let path = request.uri().path().to_string();
    let Some(header) = request
        .headers()
        .get("x-qg-envelope")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
    else {
        return Err(unauthorized(&path, "missing x-qg-envelope header", None));
    };
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 16 * 1024 * 1024)
        .await
        .map_err(|error| unauthorized(&path, &format!("unreadable body: {error}"), None))?;
    let envelope = PyTypeDidEnvelope::from_json(&header)
        .map_err(|error| unauthorized(&path, &format!("unparseable envelope: {error}"), None))?;
    let verification = envelope.verify();
    let body_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let checks = json!({
        "signatureValid": verification.signature_valid,
        "actionIsInvoke": envelope.action == "invoke",
        "resourceBoundToPath": envelope.resource == path,
        "bodyBound": envelope.payload["bodySha256"] == json!(body_sha256),
    });
    let allowed = checks
        .as_object()
        .expect("checks object")
        .values()
        .all(|value| value == &json!(true));
    if !allowed {
        return Err(unauthorized(&path, "envelope auth failed", Some(checks)));
    }
    Ok(next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await)
}

fn unauthorized(path: &str, reason: &str, checks: Option<Value>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": reason,
            "receipt": {
                "path": path,
                "allowed": false,
                "checks": checks,
                "contract": {
                    "header": "x-qg-envelope",
                    "action": "invoke",
                    "resource": "<request path>",
                    "payload": {"bodySha256": "<sha256 hex of request body>"},
                    "signature": "ed25519 over querygraph-typedid-signing-v1",
                },
            },
        })),
    )
}

async fn agent_card(headers: axum::http::HeaderMap) -> Json<Value> {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost:8080");
    Json(crate::a2a::agent_card(&format!("http://{host}")))
}

pub async fn serve(port: u16, require_auth: bool) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    eprintln!(
        "qg-server listening on http://0.0.0.0:{port}/v1{}",
        if require_auth {
            " (TypeDID envelope auth required)"
        } else {
            ""
        }
    );
    axum::serve(listener, router_with_options(require_auth)).await?;
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
struct AnswerRequest {
    question: String,
}

/// First slice of the documented `POST /v1/answer`: semantic search over the
/// registry, SQL plans for the matches, deterministic synthesis, and a signed
/// TypeDID envelope plus an OpenLineage run with a spec-conformant UUID.
/// (The fully governed loop with RBAC+ODRL receipts and pluggable LLMs is
/// qg-python's `GovernedNavigatorLoop`; parity here follows with envelope
/// auth.)
async fn answer(
    State(registry): State<ModelRegistry>,
    Json(request): Json<AnswerRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let question = request.question;
    let models: Vec<OsiDocument> = registry
        .read()
        .expect("registry lock")
        .values()
        .cloned()
        .collect();
    tokio::task::spawn_blocking(move || answer_over_models(&models, &question))
        .await
        .map_err(internal_error)?
        .map(Json)
        .map_err(internal_error)
}

/// The deterministic answer core shared by `POST /v1/answer` and the MCP
/// `answer_question` tool: search, plan, synthesize, sign, and attach the
/// OpenLineage run. Blocking (envelope signing); callers off-load as needed.
pub(crate) fn answer_over_models(models: &[OsiDocument], question: &str) -> Result<Value> {
    let needles: Vec<String> = question
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|term| term.len() > 2)
        .map(str::to_string)
        .collect();
    let mut matches = Vec::new();
    let mut plans: Vec<Value> = Vec::new();
    for document in models {
        let model = &document.semantic_model;
        let mut hit_datasets = std::collections::BTreeSet::new();
        for needle in &needles {
            for hit in search_model(model, needle) {
                if hit["kind"] == "dataset" {
                    hit_datasets.insert(hit["name"].as_str().unwrap_or_default().to_string());
                }
                if hit["kind"] == "field" {
                    hit_datasets.insert(hit["dataset"].as_str().unwrap_or_default().to_string());
                }
                if !matches.contains(&hit) {
                    matches.push(hit);
                }
            }
        }
        for dataset in &model.datasets {
            if hit_datasets.contains(&dataset.name) {
                let columns: Vec<String> = dataset
                    .fields
                    .iter()
                    .map(|field| format!("`{}`", field.name))
                    .collect();
                let selection = if columns.is_empty() {
                    "*".to_string()
                } else {
                    columns.join(", ")
                };
                plans.push(json!({
                    "dataset": dataset.name,
                    "source": dataset.source,
                    "sql": format!("SELECT {selection} FROM {}", dataset.source),
                }));
            }
        }
    }

    let sources: Vec<String> = plans
        .iter()
        .filter_map(|plan| plan["source"].as_str().map(str::to_string))
        .collect();
    let answer_text = if plans.is_empty() {
        format!("No governed sources matched {question:?}; no data may be consulted.")
    } else {
        format!(
            "Answerable from governed sources {} via {} planned quer{}.",
            sources.join(", "),
            plans.len(),
            if plans.len() == 1 { "y" } else { "ies" },
        )
    };

    let payload = json!({
        "question": question,
        "answer": answer_text.clone(),
        "synthesizedBy": "deterministic",
        "plans": plans.clone(),
    });
    let envelope = TypeDidEnvelope::from_typesec_between(
        "querygraph.answer",
        "qg-answer",
        "models:registry",
        b"querygraph-navigator",
        b"querygraph-supervisor",
        &payload,
    )?;

    let openlineage = json!({
        "eventType": "COMPLETE",
        "eventTime": chrono::Utc::now(),
        "run": {"runId": run_id_for(&envelope.signature)},
        "job": {"namespace": "querygraph", "name": "qg-rust-answer"},
        "inputs": sources.iter().map(|s| json!({"namespace": "sail", "name": s})).collect::<Vec<_>>(),
        "outputs": [json!({"namespace": "querygraph", "name": format!("querygraph:answer:{}", &envelope.payload_sha256[..16])})],
        "producer": "https://querygraph.ai/qg-rust",
        "schemaURL": "https://openlineage.io/spec/2-0-2/OpenLineage.json",
    });
    Ok(json!({
        "question": question,
        "answer": answer_text,
        "synthesizedBy": "deterministic",
        "matches": matches,
        "plans": plans,
        "envelope": envelope,
        "openlineage": openlineage,
    }))
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
pub(crate) fn search_model(model: &OsiSemanticModel, needle: &str) -> Vec<Value> {
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
    async fn envelope_auth_gates_governed_routes() {
        let router = router_with_options(true);
        let body = json!({"question": "what is fiscal capacity?"});
        let body_text = body.to_string();

        // No header → 401 with a contract receipt.
        let response = router
            .clone()
            .oneshot(post_json("/v1/answer", body.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // A properly bound, signed envelope → 200.
        let body_sha256 = format!("{:x}", sha2::Sha256::digest(body_text.as_bytes()));
        let envelope = PyTypeDidEnvelope::signed(
            "querygraph-agent:ApiClient",
            "did:web:qg-server",
            "invoke",
            "/v1/answer",
            json!({"bodySha256": body_sha256}),
        );
        let authed = Request::post("/v1/answer")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-qg-envelope", serde_json::to_string(&envelope).unwrap())
            .body(Body::from(body_text.clone()))
            .unwrap();
        let response = router.clone().oneshot(authed).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // The same envelope replayed against a different path → 401.
        let wrong_path = Request::post("/v1/models/import/osi")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-qg-envelope", serde_json::to_string(&envelope).unwrap())
            .body(Body::from(body_text))
            .unwrap();
        let response = router.clone().oneshot(wrong_path).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Open routes stay open.
        let response = router
            .oneshot(Request::get("/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn answer_plans_over_registry_and_signs_the_result() {
        let router = router();
        let croissant = json!({
            "name": "Energy Burden",
            "description": "Demo energy fields",
            "recordSet": [{
                "name": "observations",
                "field": [{"name": "monthly_cost", "description": "Monthly energy cost"}],
            }],
        });
        router
            .clone()
            .oneshot(post_json("/v1/models/import/croissant", croissant))
            .await
            .unwrap();

        let response = router
            .oneshot(post_json(
                "/v1/answer",
                json!({"question": "What drives monthly energy burden?"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(body["answer"].as_str().unwrap().contains("energy_burden"));
        assert_eq!(
            body["plans"][0]["sql"],
            "SELECT `monthly_cost` FROM sail.qg_lakehouse.energy_burden"
        );
        assert!(!body["envelope"]["signature"].as_str().unwrap().is_empty());
        // runId must be a spec-conformant UUID.
        let run_id = body["openlineage"]["run"]["runId"].as_str().unwrap();
        assert_eq!(run_id.len(), 36);
        assert_eq!(run_id.matches('-').count(), 4);
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
