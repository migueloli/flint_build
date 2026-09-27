use crate::config::PluginConfig;
use crate::parser::dart_types::{DartClass, DartEnum, ParsedFile};
use tera::{Context, Tera};

pub mod flint_json;
pub mod generic;

pub trait Generator: Send + Sync {
    /// Renders this plugin's section of `<file>.g.dart`. The engine adds the file header and the
    /// `part of` directive (spec 0001), so the section contains only generated declarations.
    fn generate(&self, filename: &str, parsed_file: ParsedFile, plugin: &PluginConfig) -> String;
}

fn class_matches(class: &DartClass, plugin: &PluginConfig) -> bool {
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
    ) {
        if let Some(path) = custom_path {
            self.tera.add_template_file(path, Some(name)).unwrap();
        } else {
            self.tera.add_raw_template(name, default_template).unwrap();
        }
    }

    pub fn load_template_file(&mut self, name: &str, path: &str) {
        self.tera
            .add_template_file(path, Some(name))
            .expect("Failed to load template file");
    }

    pub fn render(&self, name: &str, context: &Context) -> String {
        self.tera
            .render(name, context)
            .expect("Template render failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_engine_raw() {
        let mut engine = TemplateEngine::default();
        engine.load_template("test_tpl", "Hello {{ name }}", None);

        let mut context = tera::Context::new();
        context.insert("name", "Flint");

        let result = engine.render("test_tpl", &context);
        assert_eq!(result, "Hello Flint");
    }
}
