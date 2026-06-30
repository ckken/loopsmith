use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use loopsmith::{
    config::LoopConfig,
    run_state::{ApplyOutcome, RunInspection, apply_run, diff_run, inspect_run},
    runner::run_loop,
};
use std::{path::PathBuf, process::Command};

#[derive(Debug, Parser)]
#[command(name = "loopsmith")]
#[command(about = "Run auditable iterative repair loops with Codex CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Doctor,
    Run {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "runs")]
        runs_dir: PathBuf,
    },
    Inspect {
        run_id: Option<String>,
        #[arg(long, default_value = "runs")]
        runs_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Diff {
        run_id: Option<String>,
        #[arg(long)]
        iteration: Option<usize>,
        #[arg(long, default_value = "runs")]
        runs_dir: PathBuf,
    },
    Apply {
        run_id: Option<String>,
        #[arg(long)]
        iteration: Option<usize>,
        #[arg(long, default_value = "runs")]
        runs_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        verify: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor => doctor(),
        Commands::Run { config, runs_dir } => {
            let config = LoopConfig::from_path(&config)?;
            let summary = run_loop(&config, &std::env::current_dir()?, &runs_dir)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            if summary.passed {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Commands::Inspect {
            run_id,
            runs_dir,
            json,
        } => {
            let inspection = inspect_run(&runs_dir, run_id.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                print_inspection(&inspection);
            }
            Ok(())
        }
        Commands::Diff {
            run_id,
            iteration,
            runs_dir,
        } => {
            let diff = diff_run(
                &std::env::current_dir()?,
                &runs_dir,
                run_id.as_deref(),
                iteration,
            )?;
            print!("{}", diff.diff);
            Ok(())
        }
        Commands::Apply {
            run_id,
            iteration,
            runs_dir,
            dry_run,
            force,
            verify,
        } => {
            let outcome = apply_run(
                &std::env::current_dir()?,
                &runs_dir,
                run_id.as_deref(),
                iteration,
                dry_run,
                force,
                verify,
            )?;
            print_apply_outcome(&outcome);
            if outcome
                .verification
                .as_ref()
                .is_some_and(|validation| !validation.passed)
            {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

fn doctor() -> Result<()> {
    let output = Command::new("codex")
        .arg("--version")
        .output()
        .context("failed to run codex --version")?;

    if !output.status.success() {
        anyhow::bail!("codex --version failed");
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn print_inspection(inspection: &RunInspection) {
    let manifest = &inspection.manifest;
    println!("Run: {}", manifest.run_id);
    println!("Status: {:?}", manifest.status);
    println!("Artifact: {}", manifest.artifact);
    println!("Goal: {}", manifest.goal);
    println!("Iterations: {}", manifest.iterations);
    if let Some(path) = &manifest.final_artifact_path {
        println!("Final artifact: {path}");
    }
    if let Some(path) = &manifest.summary_path {
        println!("Summary: {path}");
    }
    for record in &inspection.records {
        println!(
            "Iteration {}: passed={} returncode={}",
            record.iteration, record.validation.passed, record.validation.returncode
        );
    }
}

fn print_apply_outcome(outcome: &ApplyOutcome) {
    println!("Run: {}", outcome.run_id);
    println!("Iteration: {}", outcome.iteration);
    println!("Artifact: {}", outcome.artifact);
    println!("Source: {}", outcome.source_path.display());
    println!("Candidate: {}", outcome.candidate_path.display());
    println!("Dry run: {}", outcome.dry_run);
    println!("Applied: {}", outcome.applied);
    if let Some(validation) = &outcome.verification {
        println!(
            "Verification: passed={} returncode={}",
            validation.passed, validation.returncode
        );
    }
}
