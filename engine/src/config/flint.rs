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
    pub template_path: Option<String>,
    pub converters: Option<Vec<String>>,
    pub field_rename: Option<String>,
}

impl FlintConfig {
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let config: FlintConfig = serde_yaml::from_str(content)?;
        Ok(config)
    }

    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_flint_config() {
        let yaml = r#"
            plugins:
              flint_json:
                class_annotations: ["@FlintModel"]
                field_annotations: ["@JsonKey"]
                enum_annotations: ["@JsonEnum"]
                variant_annotations: ["@JsonValue"]
                field_rename: "snake_case"
        "#;
        let config = FlintConfig::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();

        assert!(plugins.contains_key("flint_json"));
        let plugin = &plugins["flint_json"];
        assert_eq!(plugin.class_annotations, vec!["@FlintModel".to_string()]);
        assert_eq!(plugin.field_rename, Some("snake_case".to_string()));
    }

    #[test]
    fn test_parse_invalid_flint_config() {
        let yaml = r#"
            plugins:
              flint_json:
                class_annotations: "should_be_a_list_not_a_string"
        "#;
        assert!(FlintConfig::from_str(yaml).is_err());
    }
}
