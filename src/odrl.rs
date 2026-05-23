use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Action {
    Use,
    Read,
    Derive,
    Translate,
    Index,
}

impl Action {
    fn iri(&self) -> &'static str {
        match self {
            Self::Use => "odrl:use",
            Self::Read => "odrl:read",
            Self::Derive => "odrl:derive",
            Self::Translate => "querygraph:translate",
            Self::Index => "querygraph:index",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rule {
    pub action: Action,
    pub assignee: String,
    pub constraint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub id: String,
    pub target: String,
    pub assigner: String,
    pub permissions: Vec<Rule>,
    pub prohibitions: Vec<Rule>,
}

impl Policy {
    pub fn allows(&self, assignee: &str, action: &Action) -> bool {
        let prohibited = self
            .prohibitions
            .iter()
            .any(|rule| rule.assignee == assignee && &rule.action == action);
        let permitted = self
            .permissions
            .iter()
            .any(|rule| rule.assignee == assignee && &rule.action == action);
        permitted && !prohibited
    }

    pub fn to_json_ld(&self) -> Value {
        json!({
            "@type": "odrl:Policy",
            "@id": self.id,
            "odrl:target": self.target,
            "odrl:assigner": self.assigner,
            "odrl:permission": self.permissions.iter().map(rule_json).collect::<Vec<_>>(),
            "odrl:prohibition": self.prohibitions.iter().map(rule_json).collect::<Vec<_>>()
        })
    }
}

fn rule_json(rule: &Rule) -> Value {
    json!({
        "odrl:action": rule.action.iri(),
        "odrl:assignee": rule.assignee,
        "odrl:constraint": rule.constraint
    })
}
