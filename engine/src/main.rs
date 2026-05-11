use std::{fs, time::Instant};

use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use flint_build::config::FlintConfig;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use flint_build::config;
use flint_build::discovery;
use flint_build::generators;
use flint_build::parser;
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

    match &cli.command {
        Commands::Build {
            delete_conflicting_outputs,
        } => run_build(*delete_conflicting_outputs)?,
        Commands::Watch {
            delete_conflicting_outputs,
        } => watcher::watch("lib", || run_build(*delete_conflicting_outputs))?,
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

fn run_build(delete_conflicting_outputs: bool) -> Result<()> {
    let pubspec = config::Pubspec::load()?;
    log::info!("Initializing build for package: {}", pubspec.name);
    println!(
        "{} {} {}",
        "🚀".bold(),
        "Building project:".green().bold(),
        pubspec.name.cyan().bold()
    );

    let config = FlintConfig::load_from_file("flint.yaml").context("No 'flint.yaml' found in the current directory. Please create one to configure your plugins.")?;
    let total_generated = AtomicUsize::new(0);

    if let Some(plugins) = config.plugins {
        for (plugin_name, plugin_config) in plugins {
            println!("🚀 Running Plugin: {}", plugin_name);

            let files = discovery::find_dart_files("lib");
            log::debug!("Discovery found {} potential Dart files", files.len());
            println!("{} Found {} .dart files", "🔍".blue(), files.len());

            let result = files.par_iter().try_for_each(|path| -> Result<()> {
                let output_path = path.with_extension("g.dart");
                if output_path.exists() && !delete_conflicting_outputs {
                    let input_meta = fs::metadata(path)?;
                    let output_meta = fs::metadata(&output_path)?;

                    if input_meta.modified()? <= output_meta.modified()? {
                        println!(
                            "  {} Skipping {}",
                            "⚠️".yellow(),
                            path.display().to_string().dimmed()
                        );
                        return Ok(());
                    }
                }

                let parsed_file = parser::parse_file(path, &plugin_config)?;
                if !parsed_file.classes.is_empty() {
                    total_generated.fetch_add(1, Ordering::SeqCst);
                    let filename = path.file_name().unwrap().to_str().unwrap();
                    let generated = generators::flint_json::emitter::generate_full_file(
                        filename,
                        parsed_file,
                        &plugin_config.template_path,
                    );
                    fs::write(&output_path, generated)?;
                    println!(
                        "  {} Generated: {}",
                        "✅".green(),
                        output_path.display().to_string().bold()
                    );
                }
                Ok(())
            });

            if result.is_err() {
                return Err(result.err().unwrap());
            }
        }
    }

    if total_generated.load(Ordering::SeqCst) == 0 {
        println!("{} No annotations found. Nothing to build.", "ℹ️".yellow());
    } else {
        println!(
            "{} Built {}",
            "✅".green(),
            total_generated.load(Ordering::SeqCst)
        );
    }

    Ok(())
}

fn run_clean() -> Result<()> {
    println!(
        "{} {}",
        "🧹".magenta(),
        "Cleaning generated files...".bold()
    );
    let files = discovery::find_generated_files("lib");
    files.par_iter().try_for_each(|path| -> Result<()> {
        fs::remove_file(path)?;
        println!(
            "  {} Deleted: {}",
            "🗑️".red().dimmed(),
            path.display().to_string().dimmed()
        );
        Ok(())
    })
}
