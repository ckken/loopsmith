use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn prepare_iteration_workspace(
    source_root: &Path,
    artifact: &str,
    iteration_dir: &Path,
) -> Result<PathBuf> {
    let workspace = iteration_dir.join("workspace");
    copy_workspace(source_root, &workspace)?;

    let target = workspace.join(artifact);
    if !target.exists() {
        anyhow::bail!("artifact {} was not copied into workspace", artifact);
    }
    Ok(target)
}

fn copy_workspace(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("failed to clean {}", destination.display()))?;
    }
    fs::create_dir_all(destination)?;
    copy_dir_contents(source, destination)
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if should_skip(&name.to_string_lossy()) {
            continue;
        }

        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_contents(&path, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
        }
    }
    Ok(())
}

fn should_skip(name: &str) -> bool {
    matches!(name, ".git" | "target") || name.starts_with("runs") || name == ".DS_Store"
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn copies_artifact_into_iteration_workspace() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/cache"), "ignored").unwrap();

        let copied =
            prepare_iteration_workspace(dir.path(), "README.md", &dir.path().join("runs/it1"))
                .unwrap();

        assert_eq!(fs::read_to_string(copied).unwrap(), "hello");
        assert!(!dir.path().join("runs/it1/workspace/target").exists());
    }

    #[test]
    fn copies_nested_artifact_into_iteration_workspace() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs/reference")).unwrap();
        fs::write(dir.path().join("docs/reference/guide.md"), "nested").unwrap();

        let copied = prepare_iteration_workspace(
            dir.path(),
            "docs/reference/guide.md",
            &dir.path().join("runs/it1"),
        )
        .unwrap();

        assert_eq!(fs::read_to_string(copied).unwrap(), "nested");
        assert!(
            dir.path()
                .join("runs/it1/workspace/docs/reference")
                .exists()
        );
    }

    #[test]
    fn skips_git_target_and_runs_directories() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::create_dir_all(dir.path().join("runs")).unwrap();
        fs::create_dir_all(dir.path().join("runs-installed-real")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join(".git/config"), "ignored").unwrap();
        fs::write(dir.path().join("target/cache"), "ignored").unwrap();
        fs::write(dir.path().join("runs/record.json"), "ignored").unwrap();
        fs::write(
            dir.path().join("runs-installed-real/record.json"),
            "ignored",
        )
        .unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn ok() {}").unwrap();

        prepare_iteration_workspace(dir.path(), "src/lib.rs", &dir.path().join("runs-new/it1"))
            .unwrap();

        let workspace = dir.path().join("runs-new/it1/workspace");
        assert!(workspace.join("src/lib.rs").exists());
        assert!(!workspace.join(".git").exists());
        assert!(!workspace.join("target").exists());
        assert!(!workspace.join("runs").exists());
        assert!(!workspace.join("runs-installed-real").exists());
    }

    #[test]
    fn errors_when_artifact_is_missing_after_copy() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();

        let err = prepare_iteration_workspace(
            dir.path(),
            "docs/missing.md",
            &dir.path().join("runs/it1"),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("artifact docs/missing.md was not copied"));
    }
}
