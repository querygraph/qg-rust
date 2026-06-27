use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use calamine::{Data, Reader, open_workbook_auto};
use reqwest::blocking::Client;

use crate::dataverse::DataverseFile;
use crate::sail::safe_sql_name;

#[derive(Debug, Clone)]
pub(crate) struct NormalizedTabularFile {
    pub(crate) data_path: PathBuf,
    pub(crate) filename: String,
    pub(crate) delimiter: u8,
}

pub(crate) fn normalize_tabular_file(
    file: &DataverseFile,
    local_path: &Path,
    prepared_dir: &Path,
) -> Result<Option<NormalizedTabularFile>> {
    let name = file.filename.to_ascii_lowercase();
    let content_type = file
        .content_type
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if spec_is_codata(file) {
        let out = prepared_dir.join("codata_constants_2022").join("data.csv");
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        normalize_codata_constants(local_path, &out)?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out,
            filename: "codata_constants_2022.csv".to_string(),
            delimiter: b',',
        }));
    }
    if name.ends_with(".csv") || content_type.contains("text/csv") {
        let out = prepared_copy(local_path, prepared_dir, &file.filename, "csv")?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out.join(format!("data.{}", "csv")),
            filename: file.filename.clone(),
            delimiter: b',',
        }));
    }
    if name.ends_with(".tab")
        || name.ends_with(".tsv")
        || content_type.contains("tab-separated")
        || content_type.contains("text/tab")
    {
        let out = prepared_copy(local_path, prepared_dir, &file.filename, "tsv")?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out.join(format!("data.{}", "tsv")),
            filename: file.filename.clone(),
            delimiter: b'\t',
        }));
    }
    if name.ends_with(".xlsx") {
        let out_dir = prepared_dir.join(safe_sql_name(
            Path::new(&file.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file.id),
        ));
        fs::create_dir_all(&out_dir)?;
        let out = out_dir.join(format!(
            "{}.csv",
            safe_sql_name(
                Path::new(&file.filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&file.id)
            )
        ));
        xlsx_first_sheet_to_csv(local_path, &out)?;
        return Ok(Some(NormalizedTabularFile {
            data_path: out,
            filename: format!("{}.csv", file.filename),
            delimiter: b',',
        }));
    }
    Ok(None)
}

fn prepared_copy(
    source: &Path,
    prepared_dir: &Path,
    filename: &str,
    extension: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(prepared_dir)?;
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("table");
    let out_dir = prepared_dir.join(safe_sql_name(stem));
    fs::create_dir_all(&out_dir)?;
    let out = out_dir.join(format!("data.{}", extension));
    if !out.exists() || fs::metadata(&out)?.len() == 0 {
        fs::copy(source, &out)?;
    }
    Ok(out_dir)
}

fn spec_is_codata(file: &DataverseFile) -> bool {
    file.id == "codata_constants_2022" || file.filename.contains("codata_constants")
}

pub(crate) fn download_if_missing(url: &str, path: &Path, api_token: Option<&str>) -> Result<()> {
    if path.exists() && fs::metadata(path)?.len() > 0 {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("download");
    let client = Client::builder()
        .timeout(Duration::from_secs(1800))
        .build()?;
    let mut request = client.get(url);
    if let Some(api_token) = api_token {
        request = request.header("X-Dataverse-key", api_token);
    }
    let mut response = request
        .send()
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?;
    let mut out = File::create(&tmp)?;
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
    }
    out.flush()?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn xlsx_first_sheet_to_csv(path: &Path, out: &Path) -> Result<()> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .context("workbook has no sheets")?;
    let range = workbook.worksheet_range(&sheet)?;
    let mut writer = csv::Writer::from_path(out)?;
    for row in range.rows() {
        writer.write_record(row.iter().map(cell_to_string))?;
    }
    writer.flush()?;
    Ok(())
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("{value:?}"),
    }
}

fn normalize_codata_constants(path: &Path, out: &Path) -> Result<()> {
    let reader = BufReader::new(File::open(path)?);
    let mut writer = csv::Writer::from_path(out)?;
    writer.write_record(["quantity", "value", "uncertainty", "unit"])?;
    let mut in_table = false;
    for line in reader.lines() {
        let line = line?;
        if line.starts_with("----") {
            in_table = true;
            continue;
        }
        if !in_table || line.trim().is_empty() {
            continue;
        }
        if line.len() < 90 {
            continue;
        }
        let quantity = line.get(0..60).unwrap_or("").trim();
        let value = line.get(60..85).unwrap_or("").trim();
        let uncertainty = line.get(85..110).unwrap_or("").trim();
        let unit = line.get(110..).unwrap_or("").trim();
        if !quantity.is_empty() {
            writer.write_record([quantity, value, uncertainty, unit])?;
        }
    }
    writer.flush()?;
    Ok(())
}

pub(crate) fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| if ch == '/' || ch == ':' { '_' } else { ch })
        .collect()
}
