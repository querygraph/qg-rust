# Demonstration Dataset Set

The default loader currently targets:

| ID | Source | Purpose |
|---|---|---|
| `government_finance` | `doi:10.7910/DVN/LMS8NT` | Large finance CSV/TSV stress test. |
| `roadway_lidar` | `doi:10.7910/DVN/1VT6FZ` | Geospatial raster/non-tabular asset governance. |
| `access_2018_energy` | `doi:10.7910/DVN/AHFINM` | Energy access survey microdata. |
| `dockless_transportation` | `doi:10.7910/DVN/B2LJSB` | Urban mobility and demographic features. |
| `haalsi_baseline` | `doi:10.7910/DVN/F5YHML` | Health/aging survey data with usage terms. |
| `global_party_survey` | `doi:10.7910/DVN/WMGTNS` | Social science survey and codebook. |
| `pedestrian_injury_ct` | `doi:10.7910/DVN/TXIKF9` | Transportation safety spreadsheet. |
| `energy_insecurity_covid` | `doi:10.7910/DVN/OMJWNB` | Energy justice and household survey data. |
| `climate_health_pathways` | `doi:10.7910/DVN/DHDNIC` | Climate-health tabular data and datasheet. |
| `codata_constants_2022` | NIST/CODATA ASCII table | Trusted constants, units, and measurement reference. |

The Rust manifest lives in `src/lakehouse.rs` so the CLI and tests use the same
dataset set.
