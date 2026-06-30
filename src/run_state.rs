use crate::{
    record::IterationRecord,
    verify::{VerifyResult, run_verify},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunManifest {
    pub run_id: String,
    pub artifact: String,
    pub goal: String,
    pub verify: String,
    pub max_iterations: usize,
    pub source_artifact_hash: String,
    pub started_at_unix: u64,
    pub finished_at_unix: Option<u64>,
    pub status: RunStatus,
    pub iterations: usize,
    pub final_record_path: Option<String>,
    pub final_artifact_path: Option<String>,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunIndex {
    pub runs: Vec<RunIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunIndexEntry {
    pub run_id: String,
    pub artifact: String,
    pub status: RunStatus,
    pub started_at_unix: u64,
    pub finished_at_unix: Option<u64>,
    pub iterations: usize,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunInspection {
    pub manifest: RunManifest,
    pub records: Vec<IterationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffResult {
    pub run_id: String,
    pub iteration: usize,
    pub artifact: String,
    pub source_path: PathBuf,
    pub candidate_path: PathBuf,
    pub changed: bool,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplyOutcome {
    pub run_id: String,
    pub iteration: usize,
    pub artifact: String,
    pub source_path: PathBuf,
    pub candidate_path: PathBuf,
    pub applied: bool,
    pub dry_run: bool,
    pub verification: Option<VerifyResult>,
}

impl From<&RunManifest> for RunIndexEntry {
    fn from(manifest: &RunManifest) -> Self {
        Self {
            run_id: manifest.run_id.clone(),
            artifact: manifest.artifact.clone(),
            status: manifest.status.clone(),
            started_at_unix: manifest.started_at_unix,
            finished_at_unix: manifest.finished_at_unix,
            iterations: manifest.iterations,
            summary_path: manifest.summary_path.clone(),
        }
    }
}

pub fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn artifact_hash(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(stable_hash_bytes(&bytes))
}

pub fn write_manifest_and_index(manifest: &RunManifest, runs_dir: &Path) -> Result<()> {
    let run_dir = runs_dir.join(&manifest.run_id);
    fs::create_dir_all(&run_dir)?;
    fs::write(
        run_dir.join("manifest.json"),
        serde_json::to_string_pretty(manifest)?,
    )?;

    let mut index = read_index(runs_dir)?;
    index.runs.retain(|entry| entry.run_id != manifest.run_id);
    index.runs.push(RunIndexEntry::from(manifest));
    index
        .runs
        .sort_by_key(|entry| (entry.started_at_unix, entry.run_id.clone()));
    fs::create_dir_all(runs_dir)?;
    fs::write(
        runs_dir.join("index.json"),
        serde_json::to_string_pretty(&index)?,
    )?;
    Ok(())
}

pub fn latest_run_id(runs_dir: &Path) -> Result<String> {
    let index = read_index_or_scan(runs_dir)?;
    index
        .runs
        .iter()
        .max_by_key(|entry| (entry.started_at_unix, entry.run_id.as_str()))
        .map(|entry| entry.run_id.clone())
        .with_context(|| format!("no runs found in {}", runs_dir.display()))
}

pub fn inspect_run(runs_dir: &Path, run_id: Option<&str>) -> Result<RunInspection> {
    let run_id = resolve_run_id(runs_dir, run_id)?;
    let manifest = read_manifest(runs_dir, &run_id)?;
    let mut records = Vec::new();
    for iteration in 1..=manifest.iterations {
        let path = iteration_record_path(runs_dir, &run_id, iteration);
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            records.push(serde_json::from_str(&text)?);
        }
    }
    Ok(RunInspection { manifest, records })
}

pub fn write_run_summary(runs_dir: &Path, manifest: &RunManifest) -> Result<PathBuf> {
    let inspection = inspect_run(runs_dir, Some(&manifest.run_id))?;
    let path = runs_dir.join(&manifest.run_id).join("summary.md");
    fs::write(&path, render_summary(&inspection))?;
    Ok(path)
}

pub fn diff_run(
    source_root: &Path,
    runs_dir: &Path,
    run_id: Option<&str>,
    iteration: Option<usize>,
) -> Result<DiffResult> {
    let run_id = resolve_run_id(runs_dir, run_id)?;
    let manifest = read_manifest(runs_dir, &run_id)?;
    let iteration = resolve_iteration(&manifest, iteration)?;
    let source_path = source_root.join(&manifest.artifact);
    let candidate_path = candidate_artifact_path(runs_dir, &run_id, iteration, &manifest.artifact);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let candidate = fs::read_to_string(&candidate_path)
        .with_context(|| format!("failed to read {}", candidate_path.display()))?;
    let changed = source != candidate;
    let diff = render_text_diff(&source_path, &candidate_path, &source, &candidate);

    Ok(DiffResult {
        run_id,
        iteration,
        artifact: manifest.artifact,
        source_path,
        candidate_path,
        changed,
        diff,
    })
}

pub fn apply_run(
    source_root: &Path,
    runs_dir: &Path,
    run_id: Option<&str>,
    iteration: Option<usize>,
    dry_run: bool,
    force: bool,
    verify_after: bool,
) -> Result<ApplyOutcome> {
    let run_id = resolve_run_id(runs_dir, run_id)?;
    let manifest = read_manifest(runs_dir, &run_id)?;
    let iteration = resolve_iteration(&manifest, iteration)?;
    let source_path = source_root.join(&manifest.artifact);
    let candidate_path = candidate_artifact_path(runs_dir, &run_id, iteration, &manifest.artifact);

    if !candidate_path.exists() {
        bail!("candidate artifact missing: {}", candidate_path.display());
    }

    let current_hash = artifact_hash(&source_path)?;
    if current_hash != manifest.source_artifact_hash && !force {
        bail!(
            "source artifact changed since run started; use --force to apply anyway: {}",
            source_path.display()
        );
    }

    let mut verification = None;
    let mut applied = false;
    if !dry_run {
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&candidate_path, &source_path).with_context(|| {
            format!(
                "failed to copy {} to {}",
                candidate_path.display(),
                source_path.display()
            )
        })?;
        applied = true;
        if verify_after {
            verification = Some(run_verify(&manifest.verify, source_root)?);
        }
    }

    Ok(ApplyOutcome {
        run_id,
        iteration,
        artifact: manifest.artifact,
        source_path,
        candidate_path,
        applied,
        dry_run,
        verification,
    })
}

fn read_index(runs_dir: &Path) -> Result<RunIndex> {
    let path = runs_dir.join("index.json");
    if !path.exists() {
        return Ok(RunIndex { runs: Vec::new() });
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

fn read_index_or_scan(runs_dir: &Path) -> Result<RunIndex> {
    let index = read_index(runs_dir)?;
    if !index.runs.is_empty() {
        return Ok(index);
    }

    let mut runs = Vec::new();
    if !runs_dir.exists() {
        return Ok(RunIndex { runs });
    }
    for entry in fs::read_dir(runs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.json");
        if manifest_path.exists() {
            let text = fs::read_to_string(&manifest_path)
                .with_context(|| format!("failed to read {}", manifest_path.display()))?;
            let manifest: RunManifest = serde_json::from_str(&text)?;
            runs.push(RunIndexEntry::from(&manifest));
        }
    }
    runs.sort_by_key(|entry| (entry.started_at_unix, entry.run_id.clone()));
    Ok(RunIndex { runs })
}

fn read_manifest(runs_dir: &Path, run_id: &str) -> Result<RunManifest> {
    let path = runs_dir.join(run_id).join("manifest.json");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

fn resolve_run_id(runs_dir: &Path, run_id: Option<&str>) -> Result<String> {
    match run_id {
        Some(run_id) => Ok(run_id.to_string()),
        None => latest_run_id(runs_dir),
    }
}

fn resolve_iteration(manifest: &RunManifest, iteration: Option<usize>) -> Result<usize> {
    let iteration = iteration.unwrap_or(manifest.iterations);
    if iteration == 0 || iteration > manifest.iterations {
        bail!(
            "iteration {} is outside run range 1..={}",
            iteration,
            manifest.iterations
        );
    }
    Ok(iteration)
}

fn iteration_record_path(runs_dir: &Path, run_id: &str, iteration: usize) -> PathBuf {
    runs_dir
        .join(run_id)
        .join(format!("iteration_{iteration}"))
        .join("record.json")
}

fn candidate_artifact_path(
    runs_dir: &Path,
    run_id: &str,
    iteration: usize,
    artifact: &str,
) -> PathBuf {
    runs_dir
        .join(run_id)
        .join(format!("iteration_{iteration}"))
        .join("workspace")
        .join(artifact)
}

fn render_text_diff(
    source_path: &Path,
    candidate_path: &Path,
    source: &str,
    candidate: &str,
) -> String {
    if source == candidate {
        return format!(
            "--- {}\n+++ {}\n(no changes)\n",
            source_path.display(),
            candidate_path.display()
        );
    }

    let mut diff = format!(
        "--- {}\n+++ {}\n",
        source_path.display(),
        candidate_path.display()
    );
    for line in source.lines() {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in candidate.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn render_summary(inspection: &RunInspection) -> String {
    let manifest = &inspection.manifest;
    let passed = inspection
        .records
        .last()
        .map(|record| record.validation.passed)
        .unwrap_or(false);
    let mut summary = format!(
        "# Loopsmith Run {}\n\n- Status: {:?}\n- Artifact: `{}`\n- Goal: {}\n- Iterations: {}\n- Final validation passed: {}\n",
        manifest.run_id,
        manifest.status,
        manifest.artifact,
        manifest.goal,
        manifest.iterations,
        passed
    );
    if let Some(path) = &manifest.final_artifact_path {
        summary.push_str(&format!("- Final artifact: `{path}`\n"));
    }
    if let Some(path) = &manifest.final_record_path {
        summary.push_str(&format!("- Final record: `{path}`\n"));
    }
    summary.push_str("\n## Iterations\n\n");
    for record in &inspection.records {
        summary.push_str(&format!(
            "- Iteration {}: passed={}, returncode={}\n",
            record.iteration, record.validation.passed, record.validation.returncode
        ));
        if let Some(delta) = record.remaining_delta.first() {
            let delta = delta.trim();
            if !delta.is_empty() {
                summary.push_str(&format!("  - Remaining delta: `{delta}`\n"));
            }
        }
    }
    summary
}
