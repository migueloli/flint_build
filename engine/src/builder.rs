use crate::config::{self, FlintConfig, PluginConfig, Pubspec};
use crate::discovery;
use crate::generators::generic::GenericTeraGenerator;
use crate::generators::{Generator, check_template, matches_plugin};
use crate::output;
use crate::parser;
use crate::registry::PluginRegistry;
use anyhow::{Context, Result, bail};
use colored::Colorize;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What a build did. `run_build` prints it; tests inspect it.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub sources: usize,
    pub written: Vec<PathBuf>,
    /// Regenerated with byte-identical content, so not rewritten.
    pub unchanged: usize,
    /// Skipped because the output is newer than the source.
    pub up_to_date: usize,
    /// Left untouched because a plugin that matches them has a broken template.
    pub blocked: usize,
    /// Owned outputs whose source no longer produces anything.
    pub deleted: Vec<PathBuf>,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CleanReport {
    pub deleted: Vec<PathBuf>,
    /// `.g.dart` files left alone because Flint didn't write them.
    pub skipped: Vec<PathBuf>,
}

enum PluginGenerator<'a> {
    Registered(&'a dyn Generator),
    Template(GenericTeraGenerator),
}

struct ActivePlugin<'a> {
    name: String,
    config: PluginConfig,
    generator: PluginGenerator<'a>,
    /// Its template couldn't be loaded (already reported). Files it matches are left untouched, so a
    /// typo in a template never deletes or strips outputs (spec 0004).
    broken: bool,
}

impl ActivePlugin<'_> {
    fn generator(&self) -> &dyn Generator {
        match &self.generator {
            PluginGenerator::Registered(generator) => *generator,
            PluginGenerator::Template(generator) => generator,
        }
    }
}

enum Outcome {
    UpToDate,
    Nothing,
    Written(PathBuf),
    Unchanged,
    Deleted(PathBuf),
    Warning(String),
    /// Left untouched because a matching plugin's template is broken.
    Blocked,
}

pub fn run_build(force: bool, registry: &PluginRegistry) -> Result<()> {
    let pubspec = Pubspec::load()?;
    log::info!("Initializing build for package: {}", pubspec.name);
    println!(
        "{} {} {}",
        "🚀".bold(),
        "Building project:".green().bold(),
        pubspec.name.cyan().bold()
    );

    let report = build(Path::new("."), &pubspec, force, registry)?;
    print_report(&report);
    if !report.errors.is_empty() {
        bail!("Build finished with {} error(s)", report.errors.len());
    }
    Ok(())
}

/// Runs every configured plugin over `<root>/lib` (spec 0001).
///
/// Each source is parsed once. The sections of all matching plugins are written, in config order, to one
/// `<file>.g.dart`, but only when the source declares `part '<file>.g.dart';`. Files Flint didn't write
/// are never overwritten unless `force` is set, and owned outputs that are no longer produced are deleted.
pub fn build(
    root: &Path,
    pubspec: &Pubspec,
    force: bool,
    registry: &PluginRegistry,
) -> Result<BuildReport> {
    let project = config::load_project_config(root, pubspec)?;
    let mut report = BuildReport {
        notes: project.notes,
        warnings: project.warnings,
        ..Default::default()
    };
    let plugins = active_plugins(root, project.flint, registry, &mut report);
    let inputs_changed_at = newest_shared_input(root, &plugins);

    let lib = root.join("lib");
    let mut sources = discovery::find_dart_files(&lib);
    sources.sort();
    report.sources = sources.len();

    let outcomes: Vec<Result<Outcome>> = sources
        .par_iter()
        .map(|source| process_source(source, &plugins, force, inputs_changed_at))
        .collect();
    for (source, outcome) in sources.iter().zip(outcomes) {
        match outcome {
            Ok(Outcome::UpToDate) => report.up_to_date += 1,
            Ok(Outcome::Nothing) => {}
            Ok(Outcome::Blocked) => report.blocked += 1,
            Ok(Outcome::Written(path)) => report.written.push(path),
            Ok(Outcome::Unchanged) => report.unchanged += 1,
            Ok(Outcome::Deleted(path)) => report.deleted.push(path),
            Ok(Outcome::Warning(message)) => report.warnings.push(message),
            Err(error) => report
                .errors
                .push(format!("{}: {error:#}", source.display())),
        }
    }

    let mut generated = discovery::find_generated_files(&lib);
    generated.sort();
    for path in generated {
        let orphaned = output::source_path(&path).is_some_and(|source| !source.exists());
        if orphaned && output::is_owned(&path)? {
            fs::remove_file(&path)?;
            report.deleted.push(path);
        }
    }

    Ok(report)
}

fn active_plugins<'a>(
    root: &Path,
    config: FlintConfig,
    registry: &'a PluginRegistry,
    report: &mut BuildReport,
) -> Vec<ActivePlugin<'a>> {
    let mut plugins = Vec::new();
    for (name, mut config) in config.plugins.unwrap_or_default() {
        if let Some(template_path) = &config.template_path {
            config.template_path = Some(root.join(template_path).to_string_lossy().into_owned());
        }
        // A template that can't be loaded would fail for every file; report it once and skip the plugin.
        let broken = match check_template(&name, &config) {
            Ok(()) => false,
            Err(error) => {
                report.errors.push(error.to_string());
                true
            }
        };
        let generator = match registry.get(&name) {
            Some(generator) => PluginGenerator::Registered(generator),
            None if config.template_path.is_some() => {
                report.notes.push(format!(
                    "Using the generic Tera generator for plugin '{name}'."
                ));
                PluginGenerator::Template(GenericTeraGenerator {
                    plugin_name: name.clone(),
                })
            }
            None => {
                report.warnings.push(format!(
                    "Unknown plugin '{name}' has no template_path, so it was skipped."
                ));
                continue;
            }
        };
        plugins.push(ActivePlugin {
            name,
            config,
            generator,
            broken,
        });
    }
    plugins
}

/// The latest modification time of the inputs every output depends on: the config files, templates and
/// the engine binary itself. Changing any of them makes every output out of date (a partial fix for R11).
fn newest_shared_input(root: &Path, plugins: &[ActivePlugin]) -> Option<SystemTime> {
    let mut inputs: Vec<PathBuf> = ["flint.yaml", "build.yaml", "pubspec.yaml"]
        .iter()
        .map(|name| root.join(name))
        .collect();
    inputs.extend(
        plugins
            .iter()
            .filter_map(|p| p.config.template_path.as_ref().map(PathBuf::from)),
    );
    inputs.extend(std::env::current_exe().ok());
    inputs
        .iter()
        .filter_map(|path| fs::metadata(path).and_then(|m| m.modified()).ok())
        .max()
}

fn process_source(
    source: &Path,
    plugins: &[ActivePlugin],
    force: bool,
    inputs_changed_at: Option<SystemTime>,
) -> Result<Outcome> {
    let output = output::output_path(source);
    let exists = output.exists();
    let owned = exists && output::is_owned(&output)?;
    if owned && !force && is_up_to_date(source, &output, inputs_changed_at)? {
        return Ok(Outcome::UpToDate);
    }

    let parsed = parser::parse_file(source)?;
    let filename = file_name(source);
    let output_name = file_name(&output);

    if plugins
        .iter()
        .any(|plugin| plugin.broken && matches_plugin(&parsed, &plugin.config))
    {
        return Ok(Outcome::Blocked);
    }

    let mut sections = Vec::new();
    for plugin in plugins {
        if !matches_plugin(&parsed, &plugin.config) {
            continue;
        }
        let body = plugin
            .generator()
            .generate(&filename, parsed.clone(), &plugin.config)?;
        let body = output::strip_legacy_preamble(&body);
        if !body.is_empty() {
            sections.push((plugin.name.as_str(), body));
        }
    }

    let has_part = parsed
        .part_directives
        .iter()
        .any(|uri| uri.strip_prefix("./").unwrap_or(uri) == output_name);
    if sections.is_empty() || !has_part {
        if owned {
            fs::remove_file(&output)?;
            return Ok(Outcome::Deleted(output));
        }
        if !sections.is_empty() {
            return Ok(Outcome::Warning(format!(
                "{} has annotated declarations but no `part '{output_name}';` directive, so nothing was generated for it.",
                source.display()
            )));
        }
        return Ok(Outcome::Nothing);
    }

    if exists && !owned && !force {
        bail!(
            "{} exists but wasn't generated by Flint, so it was left untouched. Delete it, or run with --force to overwrite it.",
            output.display()
        );
    }

    let content = output::assemble(&filename, &sections);
    if exists && fs::read_to_string(&output).is_ok_and(|existing| existing == content) {
        // Refresh the mtime so the next build can skip this file without regenerating it.
        fs::File::options()
            .write(true)
            .open(&output)?
            .set_modified(SystemTime::now())?;
        return Ok(Outcome::Unchanged);
    }
    fs::write(&output, content).with_context(|| format!("Failed to write {}", output.display()))?;
    Ok(Outcome::Written(output))
}

fn is_up_to_date(
    source: &Path,
    output: &Path,
    inputs_changed_at: Option<SystemTime>,
) -> Result<bool> {
    let output_modified = fs::metadata(output)?.modified()?;
    Ok(fs::metadata(source)?.modified()? <= output_modified
        && inputs_changed_at.is_none_or(|changed| changed <= output_modified))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn print_report(report: &BuildReport) {
    for note in &report.notes {
        println!("{} {}", "ℹ️".cyan(), note);
    }
    for warning in &report.warnings {
        println!("{} {}", "⚠️".yellow(), warning.yellow());
    }
    println!("{} Found {} .dart files", "🔍".blue(), report.sources);
    for path in &report.written {
        println!(
            "  {} Generated: {}",
            "✅".green(),
            path.display().to_string().bold()
        );
    }
    for path in &report.deleted {
        println!(
            "  {} Deleted stale output: {}",
            "🗑️".red(),
            path.display().to_string().dimmed()
        );
    }
    for error in &report.errors {
        eprintln!("  {} {}", "❌".red(), error.red());
    }

    let nothing_done = report.written.is_empty() && report.unchanged == 0 && report.up_to_date == 0;
    if nothing_done && report.errors.is_empty() {
        println!("{} No annotations found. Nothing to build.", "ℹ️".yellow());
    } else {
        println!(
            "{} {} generated, {} unchanged, {} up to date",
            "✅".green(),
            report.written.len(),
            report.unchanged,
            report.up_to_date
        );
    }
    if report.blocked > 0 {
        println!(
            "{} {} file(s) left unchanged because a plugin's template has errors.",
            "⚠️".yellow(),
            report.blocked
        );
    }
}

pub fn run_clean() -> Result<()> {
    println!(
        "{} {}",
        "🧹".magenta(),
        "Cleaning generated files...".bold()
    );
    let report = clean(Path::new("."))?;
    for path in &report.deleted {
        println!(
            "  {} Deleted: {}",
            "🗑️".red().dimmed(),
            path.display().to_string().dimmed()
        );
    }
    if !report.skipped.is_empty() {
        println!(
            "{} Skipped {} .g.dart file(s) not generated by Flint.",
            "ℹ️".cyan(),
            report.skipped.len()
        );
    }
    Ok(())
}

/// Deletes the `.g.dart` files under `<root>/lib` that Flint owns, and leaves every other file alone.
pub fn clean(root: &Path) -> Result<CleanReport> {
    let mut files = discovery::find_generated_files(root.join("lib"));
    files.sort();
    let mut report = CleanReport::default();
    for path in files {
        if output::is_owned(&path)? {
            fs::remove_file(&path)?;
            report.deleted.push(path);
        } else {
            report.skipped.push(path);
        }
    }
    Ok(report)
}
