//! Cross-language verification of qg-python TypeDID envelopes.
//!
//! The Python port signs envelopes with Ed25519 keys derived from agent seeds
//! (SHA-256 of the seed as the private key) over a documented canonical
//! signing payload, and publishes the verifier as a W3C `did:key`
//! `verification_method`. This module lets the Rust side verify those
//! envelopes without any shared state: resolve the `did:key`, reconstruct the
//! signing payload from the envelope fields, and check the signature.

use anyhow::{Context, Result, bail};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Version label of the qg-python envelope signing payload.
pub const PY_SIGNING_LABEL: &str = "querygraph-typedid-signing-v1";
const SIGNATURE_PREFIX: &str = "ed25519:";
const UNSIGNED_PREFIX: &str = "unsigned:sha256:";
const ED25519_MULTICODEC: [u8; 2] = [0xED, 0x01];

/// The subset of a qg-python `TypeDidEnvelope` that verification needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyTypeDidEnvelope {
    pub sender: String,
    pub recipient: String,
    pub action: String,
    pub resource: String,
    pub payload: Value,
    pub payload_sha256: String,
    pub signature: String,
    #[serde(default)]
    pub verification_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PyEnvelopeVerification {
    pub payload_hash_valid: bool,
    pub signed: bool,
    pub signature_valid: bool,
    pub verification_method: Option<String>,
    pub scheme: String,
}

impl PyTypeDidEnvelope {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("parsing qg-python TypeDID envelope JSON")
    }

    /// Mint a signed envelope in the qg-python interop format: the Ed25519
    /// key derives from `sender_seed` exactly as qg-python's
    /// `Ed25519Signer.from_seed` (SHA-256 of the seed as private key), so
    /// either side can verify what the other signs.
    pub fn signed(
        sender_seed: &str,
        recipient: &str,
        action: &str,
        resource: &str,
        payload: Value,
    ) -> Self {
        let signing_key = SigningKey::from_bytes(&Sha256::digest(sender_seed.as_bytes()).into());
        let multibase = ed25519_multibase(&signing_key.verifying_key());
        let verification_method = format!("did:key:{multibase}#{multibase}");
        let payload_sha256 = canonical_json_sha256(&payload);
        let mut envelope = Self {
            sender: format!("did:key:{multibase}"),
            recipient: recipient.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            payload,
            payload_sha256,
            signature: String::new(),
            verification_method: Some(verification_method),
        };
        let signature = signing_key.sign(envelope.signing_payload().as_bytes());
        envelope.signature = format!("{SIGNATURE_PREFIX}{}", hex_encode(&signature.to_bytes()));
        envelope
    }

    /// Reconstruct the canonical signing payload the Python side signed.
    pub fn signing_payload(&self) -> String {
        [
            PY_SIGNING_LABEL,
            &self.sender,
            &self.recipient,
            &self.action,
            &self.resource,
            &self.payload_sha256,
        ]
        .join("\n")
    }

    pub fn verify(&self) -> PyEnvelopeVerification {
        let payload_hash_valid = canonical_json_sha256(&self.payload) == self.payload_sha256;
        let signed = self.signature.starts_with(SIGNATURE_PREFIX);
        let scheme = if signed {
            "ed25519"
        } else if self.signature.starts_with(UNSIGNED_PREFIX) {
            "unsigned-digest"
        } else {
            "unknown"
        };
        let signature_valid = payload_hash_valid
            && signed
            && self
                .verification_method
                .as_deref()
                .is_some_and(|method| self.signature_matches(method));
        PyEnvelopeVerification {
            payload_hash_valid,
            signed,
            signature_valid,
            verification_method: self.verification_method.clone(),
            scheme: scheme.to_string(),
        }
    }

    fn signature_matches(&self, verification_method: &str) -> bool {
        let Ok(key) = did_key_verifying_key(verification_method) else {
            return false;
        };
        let Some(hex_signature) = self.signature.strip_prefix(SIGNATURE_PREFIX) else {
            return false;
        };
        let Ok(bytes) = hex_decode(hex_signature) else {
            return false;
        };
        let Ok(signature) = Signature::from_slice(&bytes) else {
            return false;
        };
        key.verify(self.signing_payload().as_bytes(), &signature)
            .is_ok()
    }
}

/// Resolve a `did:key:z…` identifier (optionally `#fragment`-qualified) to an
/// Ed25519 verifying key.
pub fn did_key_verifying_key(did: &str) -> Result<VerifyingKey> {
    let identifier = did.split('#').next().unwrap_or_default();
    let Some(multibase) = identifier.strip_prefix("did:key:") else {
        bail!("not a did:key identifier: {did}");
    };
    let Some(base58) = multibase.strip_prefix('z') else {
        bail!("unsupported multibase prefix in {multibase}");
    };
    let raw = bs58::decode(base58)
        .into_vec()
        .with_context(|| format!("decoding base58 key from {did}"))?;
    let key_bytes = raw.strip_prefix(&ED25519_MULTICODEC[..]).unwrap_or(&raw);
    let key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected a 32-byte Ed25519 public key in {did}"))?;
    VerifyingKey::from_bytes(&key_bytes).context("invalid Ed25519 public key")
}

/// SHA-256 of the payload rendered exactly as Python's
/// `json.dumps(payload, sort_keys=True, separators=(",", ":"))` — sorted keys,
/// compact separators, and `ensure_ascii` escaping of non-ASCII characters.
pub fn canonical_json_sha256(payload: &Value) -> String {
    let mut rendered = String::new();
    write_python_json(payload, &mut rendered);
    let mut hasher = Sha256::new();
    hasher.update(rendered.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn write_python_json(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(text) => write_python_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_python_json(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| key.as_str());
            out.push('{');
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_python_string(key, out);
                out.push(':');
                write_python_json(item, out);
            }
            out.push('}');
        }
    }
}

fn write_python_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch if ch.is_ascii() => out.push(ch),
            ch => {
                // Python json.dumps(ensure_ascii=True): BMP as \uXXXX,
                // astral as UTF-16 surrogate pairs.
                let mut buffer = [0u16; 2];
                for unit in ch.encode_utf16(&mut buffer) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
}

/// Multibase (base58btc) encoding of an Ed25519 public key with its
/// multicodec prefix — the `z6Mk…` form used in did:key.
pub fn ed25519_multibase(key: &VerifyingKey) -> String {
    let mut raw = ED25519_MULTICODEC.to_vec();
    raw.extend_from_slice(key.as_bytes());
    format!("z{}", bs58::encode(raw).into_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_decode(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        bail!("odd-length hex string");
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16).context("invalid hex in signature")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_matches_python_dumps() {
        let value = json!({
            "b": 1,
            "a": ["x", {"z": null, "y": true}],
            "unicode": "café ☕",
        });
        let mut rendered = String::new();
        write_python_json(&value, &mut rendered);
        // Matches json.dumps(value, sort_keys=True, separators=(",", ":")):
        // sorted keys, compact, non-ASCII escaped per ensure_ascii=True.
        assert_eq!(
            rendered,
            r#"{"a":["x",{"y":true,"z":null}],"b":1,"unicode":"caf\u00e9 \u2615"}"#
        );
    }

    #[test]
    fn did_key_rejects_non_did_key_identifiers() {
        assert!(did_key_verifying_key("did:oyd:zQm123").is_err());
        assert!(did_key_verifying_key("did:key:Qm123").is_err());
    }

    /// Golden fixture generated by qg-python (seed
    /// `querygraph-agent:SupervisorAgent` → sha256(seed) as the Ed25519
    /// private key), asserting the two implementations stay in lock-step.
    #[test]
    fn verifies_python_signed_envelope_fixture() {
        let envelope = PyTypeDidEnvelope::from_json(PY_FIXTURE).expect("fixture should parse");
        let report = envelope.verify();
        assert!(report.payload_hash_valid, "payload hash must recompute");
        assert!(report.signed);
        assert!(
            report.signature_valid,
            "Python signature must verify in Rust"
        );

        let mut tampered = envelope.clone();
        tampered.resource = "compartment:other".to_string();
        assert!(!tampered.verify().signature_valid);
    }

    #[test]
    fn rust_minted_envelope_matches_python_key_derivation_and_verifies() {
        let envelope = PyTypeDidEnvelope::signed(
            "querygraph-agent:SupervisorAgent",
            "did:example:recipient",
            "invoke",
            "/v1/answer",
            serde_json::json!({"bodySha256": "00"}),
        );
        // Same seed as the Python fixture → same did:key identity.
        let fixture = PyTypeDidEnvelope::from_json(PY_FIXTURE).unwrap();
        assert_eq!(
            envelope.verification_method, fixture.verification_method,
            "seed-derived did:key must match qg-python's"
        );
        assert!(envelope.verify().signature_valid);
    }

    const PY_FIXTURE: &str = include_str!("../../tests/fixtures/py_envelope.json");
}
