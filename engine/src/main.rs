use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use flint_build::builder::{run_build, run_clean, run_dump_model};
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
        /// Regenerate every file, and overwrite .g.dart files Flint didn't generate
        #[arg(
            short,
            long,
            visible_alias = "delete-conflicting-outputs",
            short_alias = 'd'
        )]
        force: bool,
    },
    /// Watch the filesystem and rebuild on changes
    Watch {
        /// Regenerate every file, and overwrite .g.dart files Flint didn't generate
        #[arg(
            short,
            long,
            visible_alias = "delete-conflicting-outputs",
            short_alias = 'd'
        )]
        force: bool,
    },
    /// Clean all generated files
    Clean,
    /// Print the generator model (spec 0007) of Dart files as JSON: what generators and templates see
    DumpModel {
        /// Files to describe, relative to the package root (default: every file under lib/)
        files: Vec<std::path::PathBuf>,
        /// Print the model's JSON Schema instead
        #[arg(long)]
        schema: bool,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let start = Instant::now();

    let cli = Cli::parse();

    let mut registry = PluginRegistry::new();
    registry.register("flint_json", Box::new(FlintJsonGenerator));

    match &cli.command {
        Commands::Build { force } => run_build(*force, &registry)?,
        Commands::Watch { force } => watcher::watch("lib", || run_build(*force, &registry))?,
        Commands::Clean => run_clean()?,
        // JSON on stdout: no timing footer.
        Commands::DumpModel { files, schema } => return run_dump_model(files, *schema),
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
