use crate::generators::Generator;
use std::collections::HashMap;

#[derive(Default)]
pub struct PluginRegistry {
    generators: HashMap<String, Box<dyn Generator>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, generator: Box<dyn Generator>) {
        self.generators.insert(name.to_string(), generator);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Generator> {
        self.generators.get(name).map(|b| b.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::parser::dart_types::ParsedFile;

    struct MockGenerator;
    impl Generator for MockGenerator {
        fn generate(
            &self,
            _filename: &str,
            _parsed_file: ParsedFile,
            _plugin: &PluginConfig,
        ) -> String {
            "MockOutput".to_string()
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = PluginRegistry::new();
        registry.register("mock", Box::new(MockGenerator));

        assert!(registry.get("mock").is_some());
        assert!(registry.get("unknown").is_none());
    }
}
