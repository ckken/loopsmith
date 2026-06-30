use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codex_loop::{config::LoopConfig, runner::run_loop};
use std::{path::PathBuf, process::Command};

#[derive(Debug, Parser)]
#[command(name = "codex-loop")]
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
