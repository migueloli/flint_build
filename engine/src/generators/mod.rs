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
