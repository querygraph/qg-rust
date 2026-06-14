use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::croissant::CroissantDataset;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CdifProfile {
    Discovery,
    Manifest,
    DataDescription,
    DataAccess,
    AccessRights,
    ControlledVocabularies,
    DataIntegration,
    Universals,
    Provenance,
}

impl CdifProfile {
    pub fn iri(self) -> &'static str {
        match self {
            Self::Discovery => "https://cdif.codata.org/profile/discovery",
            Self::Manifest => "https://cdif.codata.org/profile/manifest",
            Self::DataDescription => "https://cdif.codata.org/profile/data-description",
            Self::DataAccess => "https://cdif.codata.org/profile/data-access",
            Self::AccessRights => "https://cdif.codata.org/profile/access-rights",
            Self::ControlledVocabularies => {
                "https://cdif.codata.org/profile/controlled-vocabularies"
            }
            Self::DataIntegration => "https://cdif.codata.org/profile/data-integration",
            Self::Universals => "https://cdif.codata.org/profile/universals",
            Self::Provenance => "https://cdif.codata.org/profile/provenance",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdifDataElement {
    pub id: String,
    pub name: String,
    pub data_type: String,
    pub description: String,
    pub semantic_type: Option<String>,
    pub record_set: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdifDistribution {
    pub id: String,
    pub name: String,
    pub content_url: String,
    pub encoding_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdifAccessRights {
    pub policy_id: Option<String>,
    pub license: String,
    pub rights_statement: Option<String>,
    pub odrl_policy: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdifResource {
    pub dataset_id: String,
    pub title: String,
    pub description: String,
    pub profiles: Vec<CdifProfile>,
    pub landing_page: String,
    pub access_service: String,
    pub distributions: Vec<CdifDistribution>,
    pub data_elements: Vec<CdifDataElement>,
    pub access_rights: Option<CdifAccessRights>,
    pub temporal_coverage: Option<String>,
    pub spatial_coverage: Option<String>,
    pub units: Vec<String>,
    pub vocabularies: Vec<String>,
    pub keywords: Vec<String>,
}

impl CdifResource {
    pub fn from_croissant(
        dataset: &CroissantDataset,
        landing_page: impl Into<String>,
        access_service: impl Into<String>,
    ) -> Self {
        let distributions = dataset
            .files
            .iter()
            .map(|file| CdifDistribution {
                id: file.id.clone(),
                name: file.name.clone(),
                content_url: file.content_url.clone(),
                encoding_format: file.encoding_format.clone(),
            })
            .collect();
        let data_elements = dataset
            .record_sets
            .iter()
            .flat_map(|record_set| {
                record_set.fields.iter().map(|field| CdifDataElement {
                    id: format!("{}/field/{}", record_set.id, field.name),
                    name: field.name.clone(),
                    data_type: field.data_type.clone(),
                    description: field.description.clone(),
                    semantic_type: field.semantic_type.clone(),
                    record_set: record_set.id.clone(),
                })
            })
            .collect();

        Self {
            dataset_id: dataset.id.clone(),
            title: dataset.name.clone(),
            description: dataset.description.clone(),
            profiles: vec![
                CdifProfile::Discovery,
                CdifProfile::Manifest,
                CdifProfile::DataDescription,
                CdifProfile::DataAccess,
                CdifProfile::AccessRights,
                CdifProfile::ControlledVocabularies,
                CdifProfile::DataIntegration,
                CdifProfile::Universals,
            ],
            landing_page: landing_page.into(),
            access_service: access_service.into(),
            distributions,
            data_elements,
            access_rights: Some(CdifAccessRights {
                policy_id: None,
                license: dataset.license.clone(),
                rights_statement: Some(
                    "Access and usage must satisfy the attached ODRL/TypeSec policy before agent use."
                        .to_string(),
                ),
                odrl_policy: None,
            }),
            temporal_coverage: None,
            spatial_coverage: None,
            units: Vec::new(),
            vocabularies: dataset
                .record_sets
                .iter()
                .flat_map(|rs| rs.fields.iter())
                .filter_map(|field| field.semantic_type.clone())
                .collect(),
            keywords: dataset.keywords.clone(),
        }
    }

    pub fn with_odrl_policy(mut self, policy_id: impl Into<String>, policy: Value) -> Self {
        if let Some(access_rights) = &mut self.access_rights {
            access_rights.policy_id = Some(policy_id.into());
            access_rights.odrl_policy = Some(policy);
        }
        self
    }

    pub fn to_json_ld(&self) -> Value {
        json!({
            "@context": {
                "cdif": "https://cdif.codata.org/",
                "dcat": "http://www.w3.org/ns/dcat#",
                "dct": "http://purl.org/dc/terms/",
                "odrl": "http://www.w3.org/ns/odrl/2/"
            },
            "@type": "dcat:Dataset",
            "@id": self.dataset_id,
            "dct:title": self.title,
            "dct:description": self.description,
            "cdif:profile": self.profiles.iter().map(|profile| profile.iri()).collect::<Vec<_>>(),
            "dcat:landingPage": self.landing_page,
            "dcat:accessService": {
                "@type": "dcat:DataService",
                "endpointURL": self.access_service
            },
            "dcat:distribution": self.distributions.iter().map(|distribution| {
                json!({
                    "@type": "dcat:Distribution",
                    "@id": distribution.id,
                    "dct:title": distribution.name,
                    "dcat:downloadURL": distribution.content_url,
                    "dcat:mediaType": distribution.encoding_format
                })
            }).collect::<Vec<_>>(),
            "cdif:dataElement": self.data_elements.iter().map(|element| {
                json!({
                    "@type": "cdif:DataElement",
                    "@id": element.id,
                    "dct:title": element.name,
                    "dct:description": element.description,
                    "cdif:dataType": element.data_type,
                    "cdif:semanticType": element.semantic_type,
                    "cdif:recordSet": element.record_set
                })
            }).collect::<Vec<_>>(),
            "dct:accessRights": self.access_rights.as_ref().map(|rights| {
                json!({
                    "@type": "dct:RightsStatement",
                    "@id": rights.policy_id,
                    "dct:license": rights.license,
                    "dct:description": rights.rights_statement,
                    "odrl:policy": rights.odrl_policy
                })
            }),
            "dct:temporal": self.temporal_coverage,
            "dct:spatial": self.spatial_coverage,
            "cdif:unit": self.units,
            "cdif:controlledVocabulary": self.vocabularies,
            "dcat:keyword": self.keywords
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataverse::sample_datasets;

    #[test]
    fn cdif_projection_includes_distributions_and_data_elements() {
        let croissant = sample_datasets().remove(0).to_croissant();
        let cdif = CdifResource::from_croissant(
            &croissant,
            "https://example.test/dataset",
            "https://example.test/access",
        );

        assert_eq!(cdif.distributions.len(), 1);
        assert!(!cdif.data_elements.is_empty());
        assert!(cdif.profiles.contains(&CdifProfile::AccessRights));
    }
}
