use crate::config::PluginConfig;
use crate::generators::{Generator, TemplateEngine, retain_annotated, select_variant_values};
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
        retain_annotated(&mut parsed_file, plugin);
        select_variant_values(&mut parsed_file, plugin);

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
