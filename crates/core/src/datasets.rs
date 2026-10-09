//! Ephemeral runner data; csv/serde_json own source parsing.
use crate::Pair;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub const DATASET_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetSource {
    pub format: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Dataset {
    pub rows: Vec<BTreeMap<String, Value>>,
    pub columns: Vec<String>,
}
impl DatasetSource {
    pub fn parse(&self) -> Result<Dataset> {
        ensure!(
            self.source.len() <= DATASET_LIMIT,
            "Dataset source exceeds 1 MiB"
        );
        let text = self.source.strip_prefix('\u{feff}').unwrap_or(&self.source);
        let rows = match self.format.as_str() {
            "json" => serde_json::from_str::<Vec<BTreeMap<String, Value>>>(text)
                .map_err(|_| anyhow::anyhow!("Dataset JSON must be an array of objects"))?,
            "csv" => {
                let mut reader = csv::ReaderBuilder::new()
                    .trim(csv::Trim::Headers)
                    .from_reader(text.as_bytes());
                let headers = reader
                    .headers()
                    .map_err(|_| anyhow::anyhow!("Invalid dataset CSV headers"))?
                    .clone();
                let names = headers.iter().collect::<BTreeSet<_>>();
                ensure!(
                    !headers.is_empty()
                        && headers.len() <= 100
                        && names.len() == headers.len()
                        && names.iter().all(|key| !key.is_empty()),
                    "CSV headers must be unique and nonempty; at most 100 columns"
                );
                let mut rows = Vec::new();
                for record in reader.records() {
                    ensure!(rows.len() < 100, "Dataset exceeds 100 rows");
                    let record = record
                        .map_err(|_| anyhow::anyhow!("Invalid dataset CSV row or column count"))?;
                    rows.push(
                        headers
                            .iter()
                            .zip(record.iter())
                            .map(|(key, value)| (key.to_string(), Value::String(value.into())))
                            .collect(),
                    );
                }
                rows
            }
            _ => anyhow::bail!("Dataset format must be csv or json"),
        };
        ensure!(
            !rows.is_empty() && rows.len() <= 100,
            "Dataset requires 1 to 100 rows"
        );
        let mut columns = BTreeSet::new();
        let mut nodes = 0;
        for row in &rows {
            ensure!(
                !row.is_empty() && row.len() <= 100,
                "Dataset rows require 1 to 100 fields"
            );
            for (key, value) in row {
                ensure!(
                    !key.is_empty() && key.len() <= 1024,
                    "Invalid dataset variable name"
                );
                ensure!(
                    serde_json::to_vec(value)?.len() <= 64 * 1024,
                    "Dataset value exceeds 64 KiB"
                );
                columns.insert(key.clone());
                let mut pending = vec![(value, 0usize)];
                while let Some((value, depth)) = pending.pop() {
                    nodes += 1;
                    ensure!(
                        nodes <= 50000 && depth <= 64,
                        "Dataset complexity limit exceeded"
                    );
                    match value {
                        Value::Array(values) => {
                            pending.extend(values.iter().map(|value| (value, depth + 1)))
                        }
                        Value::Object(values) => {
                            pending.extend(values.values().map(|value| (value, depth + 1)))
                        }
                        _ => {}
                    }
                }
            }
        }
        ensure!(columns.len() <= 100, "Dataset exceeds 100 columns");
        Ok(Dataset {
            rows,
            columns: columns.into_iter().collect(),
        })
    }
}
pub fn dataset_pairs(row: &BTreeMap<String, Value>) -> Result<Vec<Pair>> {
    row.iter()
        .map(|(key, value)| {
            Ok(Pair {
                id: key.clone(),
                key: key.clone(),
                value: match value {
                    Value::String(value) => value.clone(),
                    _ => serde_json::to_string(value)?,
                },
                enabled: true,
                secret: Some(true),
                local_value: None,
            })
        })
        .collect()
}
pub fn dataset_private_values(rows: &[BTreeMap<String, Value>]) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    let mut pending = rows.iter().flat_map(|row| row.values()).collect::<Vec<_>>();
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(values) => pending.extend(values.values()),
            Value::String(value) => {
                if !value.is_empty() {
                    result.insert(value.clone());
                }
            }
            _ => {
                result.insert(serde_json::to_string(value)?);
            }
        }
        ensure!(
            result.len() <= 5000,
            "Dataset private value count exceeds execution limit"
        );
    }
    ensure!(
        result.iter().map(String::len).sum::<usize>() <= 4 * crate::MAX_VARIABLE_BYTES,
        "Dataset private values exceed execution limit"
    );
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn datasets_parse_csv_quotes_and_typed_json() {
        let csv = DatasetSource {
            format: "csv".into(),
            source: "name,value\nalpha,\"one,two\"\nbeta,\"line\nnext\"\n".into(),
        }
        .parse()
        .unwrap();
        assert_eq!(csv.rows[0]["value"], "one,two");
        assert_eq!(csv.rows[1]["value"], "line\nnext");
        let json = DatasetSource {
            format: "json".into(),
            source: r#"[{"id":2,"flag":true,"object":{"password":"nested-private"}}]"#.into(),
        }
        .parse()
        .unwrap();
        assert_eq!(json.rows[0]["id"], 2);
        assert!(
            dataset_private_values(&json.rows)
                .unwrap()
                .contains("nested-private")
        );
        assert_eq!(
            dataset_pairs(&json.rows[0])
                .unwrap()
                .iter()
                .find(|pair| pair.key == "flag")
                .unwrap()
                .value,
            "true"
        );
    }
    #[test]
    fn datasets_reject_ambiguous_csv_and_wrong_shapes() {
        for (format, source) in [
            ("csv", "name,name\na,b"),
            ("csv", "name,value\na"),
            ("json", "[1]"),
            ("json", "[]"),
        ] {
            assert!(
                DatasetSource {
                    format: format.into(),
                    source: source.into()
                }
                .parse()
                .is_err()
            );
        }
    }
}
