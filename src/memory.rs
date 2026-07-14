//! Persistent, capability-secured Marciana memory for the HTTP service.
//!
//! Storage comes from Grust's durable Turso adapter. Authorization and all
//! content rehydration remain inside TypeSec: the HTTP layer first applies the
//! deny-by-default tool-call guard, then the memory router mints the typed
//! capability and enters the vault.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use querygraph_memory::TursoMemoryStore;
use serde_json::Value;
use typesec_agent::interop::{ToolCallGuard, ToolCallRequest};
use typesec_core::policy::{PolicyEngine, RequestContext, SubjectId};
use typesec_memory::agent::{MemoryToolRouter, memory_bindings};
use typesec_memory::{MemoryError, MemoryVault};

/// The persistent memory service shared by HTTP handlers.
pub struct MemoryApi {
    guard: ToolCallGuard,
    router: MemoryToolRouter<TursoMemoryStore>,
}

/// A memory request was denied or failed after authentication.
#[derive(Debug, thiserror::Error)]
pub enum MemoryApiError {
    /// The verified TypeDID subject is not authorized for this operation.
    #[error("{0}")]
    Denied(String),
    /// The backing vault or store failed.
    #[error("{0}")]
    Failed(String),
}

impl MemoryApi {
    /// Open a bootstrapped file-backed Turso store and load an RBAC policy.
    pub fn open(database: impl AsRef<Path>, policy_yaml: &str) -> Result<Self> {
        let engine: Arc<dyn PolicyEngine> = Arc::new(
            typesec_rbac::RbacEngine::from_yaml(policy_yaml)
                .map_err(|error| anyhow::anyhow!("parsing memory RBAC policy: {error}"))?,
        );
        let store = TursoMemoryStore::open(database.as_ref())
            .map_err(|error| anyhow::anyhow!("opening memory database: {error}"))?;
        let vault = MemoryVault::new(store).with_policy(engine.clone());
        let router = MemoryToolRouter::new(vault, engine.clone());
        let guard = memory_bindings()
            .into_iter()
            .fold(ToolCallGuard::new(engine), ToolCallGuard::bind);
        Ok(Self { guard, router })
    }

    /// Execute one normalized memory tool call for a cryptographically
    /// verified subject. No caller-supplied subject reaches this method.
    pub fn execute(
        &self,
        subject: &str,
        tool_name: &str,
        arguments: Value,
        purpose: Option<&str>,
    ) -> Result<Value, MemoryApiError> {
        let context = purpose.map_or_else(RequestContext::default, |purpose| {
            RequestContext::default().with_purpose(purpose)
        });
        let guarded = self.guard.check(
            &SubjectId::from(subject),
            ToolCallRequest::new(tool_name, arguments),
            &context,
        );
        if !guarded.verdict.is_allowed() {
            return Err(MemoryApiError::Denied(
                guarded
                    .denial_message()
                    .unwrap_or_else(|| "memory request was not authorized".to_string()),
            ));
        }
        self.router
            .handle(subject, &guarded.request, &context)
            .map_err(|error| match error {
                MemoryError::PolicyDenied { .. }
                | MemoryError::Capability(_)
                | MemoryError::SpaceMismatch { .. }
                | MemoryError::AboveCeiling { .. } => MemoryApiError::Denied(error.to_string()),
                MemoryError::NotFound(_) | MemoryError::Store(_) => {
                    MemoryApiError::Failed(error.to_string())
                }
            })
    }
}
