use crate::config::{self, FlintConfig, PluginConfig, Pubspec};
use crate::discovery;
use crate::generators::generic::GenericTeraGenerator;
use crate::generators::{Generator, check_template, class_matches, matches_plugin};
use crate::index::{ResolvedType, ResolvedTypes, SymbolIndex};
use crate::output;
use crate::parser;
use crate::parser::dart_types::ParsedFile;
use crate::registry::PluginRegistry;
use anyhow::{Context, Result, bail};
use colored::Colorize;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
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

/// A source file read and parsed once: its text, syntax tree, and parsed model (for the index).
type ParsedSource = (String, tree_sitter::Tree, ParsedFile);

/// Prints the generator model (spec 0007) of `files`, or of every source under `lib/` when `files` is
/// empty, as JSON on stdout. With `schema`, prints the model's JSON Schema instead. Files that can't be read
/// or parsed are reported on stderr, and make the command fail after the others are printed.
pub fn run_dump_model(files: &[PathBuf], schema: bool) -> Result<()> {
    if schema {
        println!("{}", serde_json::to_string_pretty(&crate::model::schema())?);
        return Ok(());
    }
    let pubspec = Pubspec::load()?;
    let (dump, errors) = dump_model(Path::new("."), &pubspec.name, files)?;
    println!("{}", serde_json::to_string_pretty(&dump)?);
    for error in &errors {
        eprintln!("{} {error}", "❌".red());
    }
    if !errors.is_empty() {
        bail!("{} file(s) couldn't be described", errors.len());
    }
    Ok(())
}

/// The generator model of `files` (relative to `root`, or absolute paths inside it), or of every source
/// under `lib/` when `files` is empty, plus one message per file that couldn't be read or parsed. Types are
/// resolved through an index of `lib/` and the requested files; each file is parsed once.
pub fn dump_model(
    root: &Path,
    package: &str,
    files: &[PathBuf],
) -> Result<(crate::model::ModelDump, Vec<String>)> {
    let mut sources = discovery::find_dart_files(root.join("lib"));
    sources.sort();
    let requested: Vec<PathBuf> = if files.is_empty() {
        sources.clone()
    } else {
        let canonical_root = fs::canonicalize(root)?;
        files
            .iter()
            .map(|file| {
                if file.is_absolute() {
                    let canonical = fs::canonicalize(file)
                        .with_context(|| format!("Failed to read {}", file.display()))?;
                    let relative = canonical
                        .strip_prefix(&canonical_root)
                        .with_context(|| format!("{} isn't inside the package", file.display()))?;
                    Ok(root.join(relative))
                } else {
                    Ok(root.join(file))
                }
            })
            .collect::<Result<_>>()?
    };
    let mut all = sources;
    all.extend(requested.iter().cloned());
    all.sort();
    all.dedup();

    let parsed: Vec<(PathBuf, Result<ParsedSource>)> = all
        .into_par_iter()
        .map(|path| {
            let result = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))
                .and_then(|content| {
                    let tree = parser::dart_file::parse_tree(&content, &path)?;
                    let parsed = parser::dart_file::parsed_file(&tree, &content, &path)?;
                    Ok((content, tree, parsed))
                });
            (path, result)
        })
        .collect();
    let mut index = SymbolIndex::new(root, package);
    for (path, result) in &parsed {
        if let Ok((_, _, file)) = result {
            index.add(path, file);
        }
    }

    let mut libraries = Vec::new();
    let mut errors = Vec::new();
    for path in &requested {
        match parsed.iter().find(|(p, _)| p == path).map(|(_, r)| r) {
            Some(Ok((content, tree, _))) => libraries.push(crate::model::library(
                root, package, path, content, tree, &index,
            )),
            Some(Err(error)) => errors.push(format!("{}: {error:#}", path.display())),
            None => errors.push(format!("{}: not found", path.display())),
        }
    }
    Ok((
        crate::model::ModelDump {
            model_version: crate::model::MODEL_VERSION.to_string(),
            libraries,
        },
        errors,
    ))
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

    // Index pass (spec 0005): every file is parsed once, up front, so types can be resolved across files.
    let parsed: Vec<Result<ParsedFile>> =
        sources.par_iter().map(|s| parser::parse_file(s)).collect();
    let mut index = SymbolIndex::new(root, &pubspec.name);
    for (source, file) in sources.iter().zip(&parsed) {
        if let Ok(file) = file {
            index.add(source, file);
        }
    }

    let outcomes: Vec<(Result<Outcome>, BTreeSet<String>)> = sources
        .par_iter()
        .zip(parsed.par_iter())
        .map(|(source, parsed)| {
            let mut assumed_external = BTreeSet::new();
            let outcome = process_source(
                source,
                parsed,
                &plugins,
                &index,
                force,
                inputs_changed_at,
                &mut assumed_external,
            );
            (outcome, assumed_external)
        })
        .collect();
    // Type name without its prefix → the files that used it, for one warning per name.
    let mut assumed_external: BTreeMap<String, BTreeSet<&Path>> = BTreeMap::new();
    for (source, (outcome, assumed)) in sources.iter().zip(outcomes) {
        if outcome.is_ok() {
            for name in assumed {
                let simple = name
                    .rsplit_once('.')
                    .map_or(name.as_str(), |(_, simple)| simple);
                assumed_external
                    .entry(simple.to_string())
                    .or_default()
                    .insert(source);
            }
        }
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

    for (name, files) in &assumed_external {
        report.warnings.push(assumed_external_warning(name, files));
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
    parsed: &Result<ParsedFile>,
    plugins: &[ActivePlugin],
    index: &SymbolIndex,
    force: bool,
    inputs_changed_at: Option<SystemTime>,
    assumed_external: &mut BTreeSet<String>,
) -> Result<Outcome> {
    let output = output::output_path(source);
    let exists = output.exists();
    let owned = exists && output::is_owned(&output)?;

    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(error) => bail!("{error:#}"),
    };
    // Resolved on every build, before the up-to-date check: the output depends on the files its types are
    // declared in, and a new ambiguity must be reported even if this file didn't change (spec 0005).
    let types = resolve_field_types(source, parsed, plugins, index)?;
    let dependencies: Vec<&Path> = types.values().filter_map(|t| t.path.as_deref()).collect();
    if owned && !force && is_up_to_date(source, &output, &dependencies, inputs_changed_at)? {
        return Ok(Outcome::UpToDate);
    }

    let filename = file_name(source);
    let output_name = file_name(&output);

    if plugins
        .iter()
        .any(|plugin| plugin.broken && matches_plugin(parsed, &plugin.config))
    {
        return Ok(Outcome::Blocked);
    }

    let mut sections = Vec::new();
    for plugin in plugins {
        if !matches_plugin(parsed, &plugin.config) {
            continue;
        }
        let generated =
            plugin
                .generator()
                .generate(&filename, parsed.clone(), &plugin.config, &types)?;
        assumed_external.extend(generated.assumed_external);
        let body = output::strip_legacy_preamble(&generated.code);
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

/// Resolves every type name used by the fields of classes a plugin will generate (spec 0005). A name
/// with two visible declarations is an error for this file; one the index can't find is `unresolved`.
fn resolve_field_types(
    source: &Path,
    parsed: &ParsedFile,
    plugins: &[ActivePlugin],
    index: &SymbolIndex,
) -> Result<ResolvedTypes> {
    let mut types = ResolvedTypes::new();
    for class in parsed
        .classes
        .iter()
        .filter(|class| plugins.iter().any(|p| class_matches(class, &p.config)))
    {
        for field in &class.fields {
            for name in field.dart_type.custom_names() {
                if class.type_parameters.iter().any(|t| t == name) || types.contains_key(name) {
                    continue;
                }
                let resolved = match index.resolve(source, name) {
                    Ok(resolved) => resolved.unwrap_or_else(|| {
                        ResolvedType::unresolved(index.imports_other_packages(source))
                    }),
                    Err(ambiguity) => bail!(
                        "line {}: field '{}' of '{}' has type '{}', which is declared in {}. Use an import prefix, or `show`/`hide`, to pick one.",
                        field.line,
                        field.name,
                        class.name,
                        field.dart_type,
                        join_and(&ambiguity.files)
                    ),
                };
                types.insert(name.to_string(), resolved);
            }
        }
    }
    Ok(types)
}

/// One warning for a type name the index couldn't find, which was assumed to come from another package.
fn assumed_external_warning(name: &str, files: &BTreeSet<&Path>) -> String {
    let files: Vec<&&Path> = files.iter().collect();
    let used_in = match files.as_slice() {
        [only] => only.display().to_string(),
        [first, rest @ ..] => format!(
            "{} and {} other file{}",
            first.display(),
            rest.len(),
            if rest.len() == 1 { "" } else { "s" }
        ),
        [] => String::new(),
    };
    format!(
        "`{name}` (used in {used_in}) isn't declared in this package, so Flint assumed it's a class with `fromJson`/`toJson` from an imported package. If it is, add `{name}` to `external_types` under the plugin in flint.yaml to silence this warning."
    )
}

/// `a`, `a and b`, `a, b and c`.
fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The output is up to date if it's newer than its source, the files its types are declared in, and the
/// shared inputs (config files, templates, the engine).
fn is_up_to_date(
    source: &Path,
    output: &Path,
    dependencies: &[&Path],
    inputs_changed_at: Option<SystemTime>,
) -> Result<bool> {
    let output_modified = fs::metadata(output)?.modified()?;
    for input in std::iter::once(source).chain(dependencies.iter().copied()) {
        // A dependency that disappeared would have changed the resolution; treat it as newer.
        match fs::metadata(input).and_then(|m| m.modified()) {
            Ok(modified) if modified <= output_modified => {}
            _ => return Ok(false),
        }
    }
    Ok(inputs_changed_at.is_none_or(|changed| changed <= output_modified))
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
