use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DidDocument {
    pub id: String,
    pub controller: String,
    pub public_key_multibase: String,
    pub service_endpoint: Option<String>,
}

impl DidDocument {
    pub fn new_oyd(seed: impl AsRef<[u8]>, controller: impl Into<String>) -> Self {
        let digest = Sha256::digest(seed.as_ref());
        let fingerprint = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let id = format!("did:oyd:z{}", &fingerprint[..46]);
        Self {
            id,
            controller: controller.into(),
            public_key_multibase: format!("z{fingerprint}"),
            service_endpoint: None,
        }
    }

    pub fn with_service_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.service_endpoint = Some(endpoint.into());
        self
    }
}
