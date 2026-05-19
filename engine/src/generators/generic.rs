use crate::config::PluginConfig;
use crate::generators::{Generator, TemplateEngine};
use crate::parser::dart_types::ParsedFile;
use tera::Context;

pub struct GenericTeraGenerator {
    pub plugin_name: String,
}

impl Generator for GenericTeraGenerator {
    fn generate(
        &self,
        filename: &str,
        mut parsed_file: ParsedFile,
        plugin: &PluginConfig,
    ) -> String {
        parsed_file.classes.retain(|class| {
            class
                .metadata
                .keys()
                .any(|k| plugin.class_annotations.contains(&format!("@{}", k)))
        });

        parsed_file.enums.retain(|e| {
            e.annotations.iter().any(|a| {
                plugin
                    .enum_annotations
                    .contains(&format!("@{}", a.trim_start_matches('@')))
            })
        });

        let mut engine = TemplateEngine::new();
        if let Some(path) = &plugin.template_path {
            engine.load_template_file(&self.plugin_name, path);
        }

        let mut context = Context::new();
        context.insert("classes", &parsed_file.classes);
        context.insert("enums", &parsed_file.enums);
        context.insert("filename", filename);

        engine.render(&self.plugin_name, &context)
    }
}
