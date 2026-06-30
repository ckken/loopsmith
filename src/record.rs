use crate::verify::VerifyResult;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IterationRecord {
    pub iteration: usize,
    pub review: Value,
    pub repair: Value,
    pub validation: VerifyResult,
    pub remaining_delta: Vec<String>,
}

pub fn write_record(record: &IterationRecord, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(record)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn write_record_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("runs/iteration_1/record.json");
        let record = IterationRecord {
            iteration: 1,
            review: json!({"findings": []}),
            repair: json!({"changes_made": []}),
            validation: VerifyResult {
                passed: true,
                returncode: 0,
                stdout: String::new(),
                stderr: String::new(),
            },
            remaining_delta: vec![],
        };

        write_record(&record, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_record_preserves_auditable_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("record.json");
        let record = IterationRecord {
            iteration: 2,
            review: json!({"findings": [{"severity": "p0"}]}),
            repair: json!({"changes_made": ["updated README"]}),
            validation: VerifyResult {
                passed: false,
                returncode: 1,
                stdout: "stdout delta".to_string(),
                stderr: String::new(),
            },
            remaining_delta: vec!["stdout delta".to_string()],
        };

        write_record(&record, &path).unwrap();
        let saved: IterationRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(saved.iteration, 2);
        assert!(!saved.validation.passed);
        assert_eq!(saved.remaining_delta, vec!["stdout delta"]);
    }
}
