use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    cdif::CdifResource,
    croissant::{CroissantDataset, Field, FileObject, RecordSet},
    did::DidDocument,
    odrl::{Action, Policy, Rule},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NavigatorInput {
    pub dataset_name: String,
    pub description: String,
    pub landing_page: String,
    pub data_url: String,
    pub creator: String,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigatorOutput {
    pub generated_at: DateTime<Utc>,
    pub croissant: Value,
    pub cdif: Value,
    pub did: DidDocument,
    pub odrl: Value,
    pub bundle: Value,
}

#[derive(Debug, Default)]
pub struct AiNavigator;

impl AiNavigator {
    pub fn build(&self, input: NavigatorInput) -> NavigatorOutput {
        let did = DidDocument::new_oyd(
            format!(
                "{}:{}:{}",
                input.agent_name, input.creator, input.dataset_name
            ),
            input.agent_name.clone(),
        )
        .with_service_endpoint(input.landing_page.clone());

        let dataset_id = format!("{}/#dataset", input.landing_page.trim_end_matches('/'));
        let dataset = CroissantDataset {
            id: dataset_id.clone(),
            name: input.dataset_name.clone(),
            description: input.description.clone(),
            license: "https://creativecommons.org/licenses/by/4.0/".to_string(),
            creators: vec![input.creator.clone()],
            files: vec![FileObject {
                id: format!("{dataset_id}/file/source"),
                name: "source-data".to_string(),
                content_url: input.data_url.clone(),
                encoding_format: "application/octet-stream".to_string(),
            }],
            record_sets: vec![RecordSet {
                id: format!("{dataset_id}/recordset/default"),
                name: "default observations".to_string(),
                fields: vec![
                    Field::new(
                        "subject",
                        "sc:Text",
                        "Primary entity or observation subject",
                    )
                    .semantic_type("https://schema.org/about"),
                    Field::new("value", "sc:Text", "Observed value, label, or narrative")
                        .semantic_type("https://schema.org/value"),
                    Field::new("source", "sc:URL", "Evidence or provenance URL")
                        .semantic_type("https://schema.org/citation"),
                ],
            }],
            keywords: vec![
                "AI Navigator".to_string(),
                "Croissant".to_string(),
                "CDIF".to_string(),
                "DID".to_string(),
                "ODRL".to_string(),
            ],
        };

        let cdif = CdifResource::from_croissant(&dataset, input.landing_page, input.data_url);
        let policy = Policy {
            id: format!("{dataset_id}/policy/default"),
            target: dataset_id.clone(),
            assigner: did.id.clone(),
            permissions: vec![
                Rule {
                    action: Action::Read,
                    assignee: "public".to_string(),
                    constraint: Some("attribution required".to_string()),
                },
                Rule {
                    action: Action::Index,
                    assignee: did.id.clone(),
                    constraint: Some("local semantic indexing for AI Navigator".to_string()),
                },
            ],
            prohibitions: vec![Rule {
                action: Action::Derive,
                assignee: "public".to_string(),
                constraint: Some("no model training without separate agreement".to_string()),
            }],
        };

        let croissant_json = dataset.to_json_ld();
        let cdif_json = cdif.to_json_ld();
        let odrl_json = policy.to_json_ld();
        let generated_at = Utc::now();
        let bundle = json!({
            "@context": {
                "schema": "https://schema.org/",
                "cr": "http://mlcommons.org/croissant/",
                "cdif": "https://cdif.codata.org/",
                "dcat": "http://www.w3.org/ns/dcat#",
                "dct": "http://purl.org/dc/terms/",
                "odrl": "http://www.w3.org/ns/odrl/2/",
                "querygraph": "https://querygraph.ai/ns#"
            },
            "@type": "querygraph:AiNavigatorSemanticBundle",
            "generatedAt": generated_at,
            "identity": did,
            "layers": {
                "semanticCroissant": croissant_json,
                "cdif": cdif_json,
                "did": did,
                "odrl": odrl_json
            }
        });

        NavigatorOutput {
            generated_at,
            croissant: croissant_json,
            cdif: cdif_json,
            did,
            odrl: odrl_json,
            bundle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_all_four_semantic_layers() {
        let output = AiNavigator.build(NavigatorInput {
            dataset_name: "Hazard vocabulary".to_string(),
            description: "Controlled vocabulary with multilingual technical terms".to_string(),
            landing_page: "https://querygraph.ai/datasets/hazards".to_string(),
            data_url: "https://querygraph.ai/datasets/hazards.csv".to_string(),
            creator: "QueryGraph".to_string(),
            agent_name: "AI Navigator".to_string(),
        });

        assert_eq!(output.croissant["@type"], "cr:Dataset");
        assert_eq!(output.cdif["@type"], "dcat:Dataset");
        assert!(output.did.id.starts_with("did:oyd:z"));
        assert_eq!(output.odrl["@type"], "odrl:Policy");
    }
}
