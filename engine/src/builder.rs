use crate::config::{self, FlintConfig};
use crate::discovery;
use crate::generators::Generator;
use crate::generators::generic::GenericTeraGenerator;
use crate::parser;
use crate::registry::PluginRegistry;
use anyhow::{Context, Result};
use colored::Colorize;
use rayon::prelude::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn run_build(delete_conflicting_outputs: bool, registry: &PluginRegistry) -> Result<()> {
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

            let generic_generator;
            let generator: &dyn Generator = match registry.get(&plugin_name) {
                Some(generator) => generator,
                None => {
                    if plugin_config.template_path.is_some() {
                        println!(
                            "  {} Falling back to Generic Tera Generator for {}",
                            "⚡".cyan(),
                            plugin_name
                        );
                        generic_generator = GenericTeraGenerator {
                            plugin_name: plugin_name.clone(),
                        };
                        &generic_generator
                    } else {
                        println!(
                            "  {} Unknown plugin (and no template_path provided): {}",
                            "⚠️".yellow(),
                            plugin_name
                        );
                        continue;
                    }
                }
            };

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

                let parsed_file = parser::parse_file(path)?;
                if !parsed_file.classes.is_empty() || !parsed_file.enums.is_empty() {
                    total_generated.fetch_add(1, Ordering::SeqCst);
                    let filename = path.file_name().unwrap().to_str().unwrap();
                    let generated = generator.generate(filename, parsed_file, &plugin_config);

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

pub fn run_clean() -> Result<()> {
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
