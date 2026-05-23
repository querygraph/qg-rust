use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::croissant::CroissantDataset;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CdifProfile {
    Discovery,
    DataAccess,
    ControlledVocabularies,
    DataIntegration,
    Universals,
}

impl CdifProfile {
    pub fn iri(self) -> &'static str {
        match self {
            Self::Discovery => "https://cdif.codata.org/profile/discovery",
            Self::DataAccess => "https://cdif.codata.org/profile/data-access",
            Self::ControlledVocabularies => {
                "https://cdif.codata.org/profile/controlled-vocabularies"
            }
            Self::DataIntegration => "https://cdif.codata.org/profile/data-integration",
            Self::Universals => "https://cdif.codata.org/profile/universals",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdifResource {
    pub dataset_id: String,
    pub profiles: Vec<CdifProfile>,
    pub landing_page: String,
    pub access_service: String,
    pub temporal_coverage: Option<String>,
    pub spatial_coverage: Option<String>,
    pub units: Vec<String>,
    pub vocabularies: Vec<String>,
}

impl CdifResource {
    pub fn from_croissant(
        dataset: &CroissantDataset,
        landing_page: impl Into<String>,
        access_service: impl Into<String>,
    ) -> Self {
        Self {
            dataset_id: dataset.id.clone(),
            profiles: vec![
                CdifProfile::Discovery,
                CdifProfile::DataAccess,
                CdifProfile::ControlledVocabularies,
                CdifProfile::DataIntegration,
                CdifProfile::Universals,
            ],
            landing_page: landing_page.into(),
            access_service: access_service.into(),
            temporal_coverage: None,
            spatial_coverage: None,
            units: Vec::new(),
            vocabularies: dataset
                .record_sets
                .iter()
                .flat_map(|rs| rs.fields.iter())
                .filter_map(|field| field.semantic_type.clone())
                .collect(),
        }
    }

    pub fn to_json_ld(&self) -> Value {
        json!({
            "@type": "dcat:Dataset",
            "@id": self.dataset_id,
            "cdif:profile": self.profiles.iter().map(|profile| profile.iri()).collect::<Vec<_>>(),
            "dcat:landingPage": self.landing_page,
            "dcat:accessService": {
                "@type": "dcat:DataService",
                "endpointURL": self.access_service
            },
            "dct:temporal": self.temporal_coverage,
            "dct:spatial": self.spatial_coverage,
            "cdif:unit": self.units,
            "cdif:controlledVocabulary": self.vocabularies
        })
    }
}
