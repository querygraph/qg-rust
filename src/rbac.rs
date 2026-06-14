use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RbacPolicy {
    pub assignments: BTreeMap<String, BTreeSet<String>>,
    pub roles: BTreeMap<String, RbacRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RbacRole {
    pub id: String,
    pub permissions: Vec<RbacPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RbacPermission {
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RbacDecision {
    pub principal: String,
    pub action: String,
    pub resource: String,
    pub allowed: bool,
    pub matched_roles: Vec<String>,
}

impl RbacPolicy {
    pub fn new() -> Self {
        Self {
            assignments: BTreeMap::new(),
            roles: BTreeMap::new(),
        }
    }

    pub fn assign_role(mut self, principal: impl Into<String>, role: impl Into<String>) -> Self {
        self.assignments
            .entry(principal.into())
            .or_default()
            .insert(role.into());
        self
    }

    pub fn with_role(mut self, role: RbacRole) -> Self {
        self.roles.insert(role.id.clone(), role);
        self
    }

    pub fn decide(
        &self,
        principal: impl AsRef<str>,
        action: impl AsRef<str>,
        resource: impl AsRef<str>,
    ) -> RbacDecision {
        let principal = principal.as_ref();
        let action = action.as_ref();
        let resource = resource.as_ref();
        let mut matched_roles = Vec::new();
        if let Some(role_ids) = self.assignments.get(principal) {
            for role_id in role_ids {
                if let Some(role) = self.roles.get(role_id) {
                    if role
                        .permissions
                        .iter()
                        .any(|permission| permission.matches(action, resource))
                    {
                        matched_roles.push(role.id.clone());
                    }
                }
            }
        }
        RbacDecision {
            principal: principal.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed: !matched_roles.is_empty(),
            matched_roles,
        }
    }
}

impl Default for RbacPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RbacRole {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            permissions: Vec::new(),
        }
    }

    pub fn allow(mut self, action: impl Into<String>, resource: impl Into<String>) -> Self {
        self.permissions.push(RbacPermission {
            action: action.into(),
            resource: resource.into(),
        });
        self
    }
}

impl RbacPermission {
    fn matches(&self, action: &str, resource: &str) -> bool {
        (self.action == "*" || self.action == action)
            && (self.resource == "*" || self.resource == resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_policy_requires_assigned_matching_role() {
        let policy = RbacPolicy::new()
            .with_role(RbacRole::new("navigator").allow("answer", "dataset"))
            .assign_role("did:example:agent", "navigator");

        let allowed = policy.decide("did:example:agent", "answer", "dataset");
        let denied = policy.decide("did:example:agent", "export", "dataset");

        assert!(allowed.allowed);
        assert_eq!(allowed.matched_roles, vec!["navigator"]);
        assert!(!denied.allowed);
    }
}
