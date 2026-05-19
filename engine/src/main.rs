use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use flint_build::builder::{run_build, run_clean};
use flint_build::generators::flint_json::emitter::FlintJsonGenerator;
use flint_build::registry::PluginRegistry;
use flint_build::watcher;

#[derive(Parser)]
#[command(name = "flint_build")]
#[command(about = "⚡ A fast, native build_runner replacement", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single build
    Build {
        /// Delete conflicting outputs before building
        #[arg(short, long, default_value_t = false)]
        delete_conflicting_outputs: bool,
    },
    /// Watch the filesystem and rebuild on changes
    Watch {
        /// Delete conflicting outputs before building
        #[arg(short, long, default_value_t = false)]
        delete_conflicting_outputs: bool,
    },
    /// Clean all generated files
    Clean,
}

fn main() -> Result<()> {
    env_logger::init();
    let start = Instant::now();

    let cli = Cli::parse();

    let mut registry = PluginRegistry::new();
    registry.register("flint_json", Box::new(FlintJsonGenerator));

    match &cli.command {
        Commands::Build {
            delete_conflicting_outputs,
        } => run_build(*delete_conflicting_outputs, &registry)?,
        Commands::Watch {
            delete_conflicting_outputs,
        } => watcher::watch("lib", || run_build(*delete_conflicting_outputs, &registry))?,
        Commands::Clean => run_clean()?,
    }

    let duration = start.elapsed();
    println!(
        "\n{} {} in {:2.2?}",
        "✨".bold(),
        "Done".green().bold(),
        duration
    );
    Ok(())
}
