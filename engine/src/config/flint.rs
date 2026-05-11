use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct FlintConfig {
    pub plugins: Option<HashMap<String, PluginConfig>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PluginConfig {
    pub class_annotations: Vec<String>,
    pub field_annotations: Vec<String>,
    pub enum_annotations: Vec<String>,
    pub variant_annotations: Vec<String>,
    pub template_path: String,
}

impl FlintConfig {
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: FlintConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
