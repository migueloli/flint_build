use crate::config::PluginConfig;
use crate::error::FlintError;
use crate::index::ResolvedTypes;
use crate::parser::dart_types::{DartClass, DartEnum, ParsedFile};
use std::collections::BTreeSet;
use tera::{Context, Tera};

pub mod flint_json;
pub mod generic;

pub trait Generator: Send + Sync {
    /// Renders this plugin's section of `<file>.g.dart`. The engine adds the file header and the
    /// `part of` directive (spec 0001), so the section contains only generated declarations.
    /// `types` says what each type name used by the file's classes refers to (spec 0005).
    fn generate(
        &self,
        filename: &str,
        parsed_file: ParsedFile,
        plugin: &PluginConfig,
        types: &ResolvedTypes,
    ) -> Result<Generated, FlintError>;
}

/// A plugin's section of `<file>.g.dart`, plus what the build should tell the user about it.
#[derive(Debug, Default, PartialEq)]
pub struct Generated {
    pub code: String,
    /// Type names that aren't declared in this package or listed in `external_types`, which the generator
    /// assumed are classes with `fromJson`/`toJson` from an imported package (spec 0005). The build warns
    /// once per name.
    pub assumed_external: BTreeSet<String>,
}

impl From<String> for Generated {
    fn from(code: String) -> Self {
        Generated {
            code,
            ..Default::default()
        }
    }
}

/// Loads the plugin's `template_path` once, so a missing file or a syntax error is reported before any
/// source is processed (spec 0004).
pub fn check_template(plugin_name: &str, plugin: &PluginConfig) -> Result<(), FlintError> {
    match &plugin.template_path {
        Some(path) => TemplateEngine::new()
            .load_template_file(plugin_name, path)
            .map_err(|e| FlintError::template(plugin_name, &e)),
        None => Ok(()),
    }
}

pub fn class_matches(class: &DartClass, plugin: &PluginConfig) -> bool {
    class
        .metadata
        .keys()
        .any(|key| plugin.class_annotations.contains(&format!("@{key}")))
}

fn enum_matches(dart_enum: &DartEnum, plugin: &PluginConfig) -> bool {
    dart_enum.annotations.iter().any(|annotation| {
        plugin
            .enum_annotations
            .contains(&format!("@{}", annotation.trim_start_matches('@')))
    })
}

/// Whether any class or enum in the file carries one of the plugin's annotations.
pub fn matches_plugin(parsed_file: &ParsedFile, plugin: &PluginConfig) -> bool {
    parsed_file.classes.iter().any(|c| class_matches(c, plugin))
        || parsed_file.enums.iter().any(|e| enum_matches(e, plugin))
}

/// Sets each enum value's `value` from the first of the plugin's `variant_annotations` on it, so other
/// annotations on a constant (`@Deprecated('…')`, …) never become its JSON value.
pub fn select_variant_values(parsed_file: &mut ParsedFile, plugin: &PluginConfig) {
    for dart_enum in &mut parsed_file.enums {
        select_enum_values(dart_enum, plugin);
    }
}

/// [`select_variant_values`] for one enum, e.g. one declared in another file.
pub fn select_enum_values(dart_enum: &mut DartEnum, plugin: &PluginConfig) {
    for value in &mut dart_enum.values {
        let variant = value
            .annotations
            .iter()
            .find(|a| plugin.variant_annotations.contains(&format!("@{}", a.name)));
        value.value = variant.and_then(|a| a.value.clone());
        value.literal = variant.and_then(|a| a.literal.clone());
    }
}

/// Drops the classes and enums that don't carry one of the plugin's annotations.
pub fn retain_annotated(parsed_file: &mut ParsedFile, plugin: &PluginConfig) {
    parsed_file.classes.retain(|c| class_matches(c, plugin));
    parsed_file.enums.retain(|e| enum_matches(e, plugin));
}

pub struct TemplateEngine {
    tera: Tera,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    pub fn new() -> Self {
        Self {
            tera: Tera::default(),
        }
    }

    pub fn load_template(
        &mut self,
        name: &str,
        default_template: &str,
        custom_path: Option<&String>,
    ) -> Result<(), tera::Error> {
        match custom_path {
            Some(path) => self.load_template_file(name, path),
            None => self.tera.add_raw_template(name, default_template),
        }
    }

    pub fn load_template_file(&mut self, name: &str, path: &str) -> Result<(), tera::Error> {
        self.tera.add_template_file(path, Some(name))
    }

    pub fn render(&self, name: &str, context: &Context) -> Result<String, tera::Error> {
        self.tera.render(name, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_value_comes_only_from_variant_annotations() {
        let code = "@JsonEnum()\nenum Level {\n  @JsonValue('lo') @Note('use high') low,\n  @Note('x') mid,\n  high,\n}\n";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("level.dart");
        std::fs::write(&path, code).unwrap();
        let mut parsed = crate::parser::parse_file(&path).unwrap();
        let plugin = PluginConfig {
            enum_annotations: vec!["@JsonEnum".to_string()],
            variant_annotations: vec!["@JsonValue".to_string()],
            ..Default::default()
        };

        select_variant_values(&mut parsed, &plugin);

        let values: Vec<Option<&str>> = parsed.enums[0]
            .values
            .iter()
            .map(|v| v.value.as_deref())
            .collect();
        assert_eq!(values, vec![Some("lo"), None, None]);
    }

    #[test]
    fn test_template_engine_raw() {
        let mut engine = TemplateEngine::default();
        engine
            .load_template("test_tpl", "Hello {{ name }}", None)
            .unwrap();

        let mut context = tera::Context::new();
        context.insert("name", "Flint");

        let result = engine.render("test_tpl", &context).unwrap();
        assert_eq!(result, "Hello Flint");
    }

    #[test]
    fn test_template_errors_include_the_cause() {
        let plugin = |path: &str| PluginConfig {
            template_path: Some(path.to_string()),
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();

        let missing = dir.path().join("missing.tera");
        let error = check_template("custom", &plugin(missing.to_str().unwrap()))
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("Plugin 'custom': "), "{error}");
        assert!(error.contains("missing.tera"), "{error}");

        let broken = dir.path().join("broken.tera");
        std::fs::write(&broken, "{{ name }\n").unwrap();
        let error = check_template("custom", &plugin(broken.to_str().unwrap()))
            .unwrap_err()
            .to_string();
        // Tera's own message is only "Failed to parse …"; the cause says where and why.
        assert!(error.contains("Failed to parse"), "{error}");
        assert!(error.contains("1:"), "{error}");

        assert!(check_template("custom", &PluginConfig::default()).is_ok());
    }
}
