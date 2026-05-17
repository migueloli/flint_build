use crate::config::PluginConfig;
use crate::parser::dart_types::ParsedFile;

pub mod flint_json;

pub trait Generator: Send + Sync {
    fn generate(&self, filename: &str, parsed_file: ParsedFile, plugin: &PluginConfig) -> String;
}
