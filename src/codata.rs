use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredPayload {
    pub url: Option<String>,
    pub timestamp: Option<String>,
    pub title: Option<String>,
    pub is_rdf: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnchoredDid {
    pub did: String,
    pub doc: Option<Value>,
    pub stored_payload: Option<StoredPayload>,
}

#[derive(Debug, Clone)]
pub struct CodataOdrlClient {
    base_url: String,
}

impl Default for CodataOdrlClient {
    fn default() -> Self {
        Self::new("https://odrl.dev.codata.org")
    }
}

impl CodataOdrlClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn create_did_from_url(&self, url: &str) -> Result<AnchoredDid, reqwest::Error> {
        reqwest::blocking::Client::new()
            .get(format!("{}/api/did/create_from_url", self.base_url))
            .query(&[("url", url)])
            .send()?
            .error_for_status()?
            .json()
    }
}
