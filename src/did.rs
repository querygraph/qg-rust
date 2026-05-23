use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DidDocument {
    #[serde(rename = "@context", skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<String>>,
    pub id: String,
    pub controller: String,
    pub public_key_multibase: String,
    pub service_endpoint: Option<String>,
}

impl DidDocument {
    pub fn new_oyd(seed: impl AsRef<[u8]>, controller: impl Into<String>) -> Self {
        let digest = Sha256::digest(seed.as_ref());
        let mut multihash = Vec::with_capacity(34);
        multihash.push(0x12);
        multihash.push(0x20);
        multihash.extend_from_slice(&digest);
        let fingerprint = bs58::encode(multihash).into_string();
        let id = format!("did:oyd:z{fingerprint}");
        Self {
            context: Some(vec![
                "https://www.w3.org/ns/did/v1".to_string(),
                "https://w3id.org/security/suites/ed25519-2020/v1".to_string(),
            ]),
            id,
            controller: controller.into(),
            public_key_multibase: format!("z{}", bs58::encode(digest).into_string()),
            service_endpoint: None,
        }
    }

    pub fn with_service_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.service_endpoint = Some(endpoint.into());
        self
    }
}
