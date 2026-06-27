use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseDatasetSpec {
    pub id: String,
    pub persistent_id: Option<String>,
    pub title: String,
    pub source: LakehouseSource,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum LakehouseSource {
    Dataverse { base_url: String },
    Url { url: String, filename: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseLoadOptions {
    pub root: PathBuf,
    pub schema: String,
    pub sail_endpoint: String,
    pub max_files_per_dataset: Option<usize>,
    pub api_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseReport {
    pub root: PathBuf,
    pub schema: String,
    pub endpoint: String,
    pub datasets: Vec<LakehouseDatasetReport>,
    pub catalog_tables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseVerifyReport {
    pub endpoint: String,
    pub schema: String,
    pub typed_tables: usize,
    pub manifest_rows: i64,
    pub sail_rows: i64,
    pub tables: Vec<LakehouseVerifyTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseVerifyTable {
    pub table: String,
    pub manifest_rows: i64,
    pub sail_rows: i64,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseDatasetReport {
    pub id: String,
    pub title: String,
    pub persistent_id: Option<String>,
    pub files: Vec<LakehouseFileReport>,
    pub croissant_path: PathBuf,
    pub cdif_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LakehouseFileReport {
    pub file_id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub local_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub table: Option<String>,
    pub rows: Option<i64>,
    pub columns: Vec<TypedColumn>,
    pub parse_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypedColumn {
    pub source_name: String,
    pub name: String,
    pub data_type: LakehouseDataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LakehouseDataType {
    Boolean,
    Int64,
    Float64,
    Date,
    Timestamp,
    String,
}

impl LakehouseDataType {
    pub(crate) fn spark_type(self) -> &'static str {
        match self {
            Self::Boolean => "BOOLEAN",
            Self::Int64 => "BIGINT",
            Self::Float64 => "DOUBLE",
            Self::Date => "DATE",
            Self::Timestamp => "TIMESTAMP",
            Self::String => "STRING",
        }
    }

    pub(crate) fn croissant_type(self) -> &'static str {
        match self {
            Self::Boolean => "sc:Boolean",
            Self::Int64 => "sc:Integer",
            Self::Float64 => "sc:Float",
            Self::Date => "sc:Date",
            Self::Timestamp => "sc:DateTime",
            Self::String => "sc:Text",
        }
    }
}

pub fn default_dataset_specs() -> Vec<LakehouseDatasetSpec> {
    vec![
        dataverse_spec(
            "government_finance",
            "doi:10.7910/DVN/LMS8NT",
            "The Government Finance Database",
            "finance",
        ),
        dataverse_spec(
            "roadway_lidar",
            "doi:10.7910/DVN/1VT6FZ",
            "Roadway vulnerability LiDAR DTM",
            "geospatial",
        ),
        dataverse_spec(
            "access_2018_energy",
            "doi:10.7910/DVN/AHFINM",
            "Access to Clean Cooking Energy and Electricity: Survey of States in India 2018",
            "energy",
        ),
        dataverse_spec(
            "dockless_transportation",
            "doi:10.7910/DVN/B2LJSB",
            "Dockless transportation hotspots and mode shift",
            "transportation",
        ),
        dataverse_spec(
            "haalsi_baseline",
            "doi:10.7910/DVN/F5YHML",
            "HAALSI Baseline Survey",
            "health",
        ),
        dataverse_spec(
            "global_party_survey",
            "doi:10.7910/DVN/WMGTNS",
            "Global Party Survey, 2019",
            "social_science",
        ),
        dataverse_spec(
            "pedestrian_injury_ct",
            "doi:10.7910/DVN/TXIKF9",
            "Pedestrian injury severity in Connecticut",
            "transportation",
        ),
        dataverse_spec(
            "energy_insecurity_covid",
            "doi:10.7910/DVN/OMJWNB",
            "Energy insecurity among low-income households during COVID-19",
            "energy",
        ),
        dataverse_spec(
            "climate_health_pathways",
            "doi:10.7910/DVN/DHDNIC",
            "Exploring Climate and Health Pathways in the INSPIRE Network",
            "climate_health",
        ),
        LakehouseDatasetSpec {
            id: "codata_constants_2022".to_string(),
            persistent_id: None,
            title: "CODATA/NIST 2022 Fundamental Physical Constants".to_string(),
            category: "reference".to_string(),
            source: LakehouseSource::Url {
                url: "https://physics.nist.gov/cuu/Constants/Table/allascii.txt".to_string(),
                filename: "codata_constants_2022_allascii.txt".to_string(),
            },
        },
    ]
}

fn dataverse_spec(
    id: &str,
    persistent_id: &str,
    title: &str,
    category: &str,
) -> LakehouseDatasetSpec {
    LakehouseDatasetSpec {
        id: id.to_string(),
        persistent_id: Some(persistent_id.to_string()),
        title: title.to_string(),
        category: category.to_string(),
        source: LakehouseSource::Dataverse {
            base_url: "https://dataverse.harvard.edu".to_string(),
        },
    }
}
