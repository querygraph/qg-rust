use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub data_type: String,
    pub description: String,
    pub semantic_type: Option<String>,
}

impl Field {
    pub fn new(
        name: impl Into<String>,
        data_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            description: description.into(),
            semantic_type: None,
        }
    }

    pub fn semantic_type(mut self, semantic_type: impl Into<String>) -> Self {
        self.semantic_type = Some(semantic_type.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileObject {
    pub id: String,
    pub name: String,
    pub content_url: String,
    pub encoding_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordSet {
    pub id: String,
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CroissantDataset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub license: String,
    pub creators: Vec<String>,
    pub files: Vec<FileObject>,
    pub record_sets: Vec<RecordSet>,
    pub keywords: Vec<String>,
}

impl CroissantDataset {
    pub fn to_json_ld(&self) -> Value {
        json!({
            "@context": {
                "@vocab": "https://schema.org/",
                "cr": "http://mlcommons.org/croissant/",
                "dcat": "http://www.w3.org/ns/dcat#",
                "odrl": "http://www.w3.org/ns/odrl/2/"
            },
            "@type": "cr:Dataset",
            "@id": self.id,
            "name": self.name,
            "description": self.description,
            "license": self.license,
            "creator": self.creators.iter().map(|name| json!({"@type": "Person", "name": name})).collect::<Vec<_>>(),
            "keywords": self.keywords,
            "distribution": self.files.iter().map(|file| {
                json!({
                    "@type": "cr:FileObject",
                    "@id": file.id,
                    "name": file.name,
                    "contentUrl": file.content_url,
                    "encodingFormat": file.encoding_format
                })
            }).collect::<Vec<_>>(),
            "recordSet": self.record_sets.iter().map(|record_set| {
                json!({
                    "@type": "cr:RecordSet",
                    "@id": record_set.id,
                    "name": record_set.name,
                    "field": record_set.fields.iter().map(|field| {
                        json!({
                            "@type": "cr:Field",
                            "name": field.name,
                            "dataType": field.data_type,
                            "description": field.description,
                            "sameAs": field.semantic_type
                        })
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }
}
