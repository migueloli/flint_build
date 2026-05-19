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
