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
}
