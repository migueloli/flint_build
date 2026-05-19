use crate::config::PluginConfig;
use crate::parser::dart_types::ParsedFile;
use tera::{Context, Tera};

pub mod flint_json;
pub mod generic;

pub trait Generator: Send + Sync {
    fn generate(&self, filename: &str, parsed_file: ParsedFile, plugin: &PluginConfig) -> String;
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
        let mut engine = TemplateEngine::new();
        // Load a raw string template
        engine.load_template("test_tpl", "Hello {{ name }}", None);

        let mut context = tera::Context::new();
        context.insert("name", "Flint");

        let result = engine.render("test_tpl", &context);
        assert_eq!(result, "Hello Flint");
    }
}
