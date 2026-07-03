//! First slice of the documented `/v1` HTTP API
//! (`docs/sail-typesec-grust-implementation.md` §"API Surface").
//!
//! Makes the governed semantic layer reachable over a network: build
//! four-layer Navigator bundles, run the QGLake governance story, and verify
//! TypeDID envelopes (including qg-python's Ed25519 envelopes). Verification
//! results and policy denials are first-class 200 responses carrying
//! receipts — an invalid signature is a finding, not a server error.

use anyhow::Result;
use axum::{
    Json, Router,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    agent::{PyEnvelopeVerification, PyTypeDidEnvelope},
    navigator::{AiNavigator, NavigatorInput},
    qglake::run_qglake_story,
};

pub fn router() -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/navigator/bundle", post(navigator_bundle))
        .route("/v1/qglake/story", get(qglake_story))
        .route("/v1/audit/verify-envelope", post(verify_envelope))
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
