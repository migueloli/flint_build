use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Pubspec {
    pub name: String,
    dependencies: Option<HashMap<String, Value>>,
    dev_dependencies: Option<HashMap<String, Value>>,
}

impl Pubspec {
    /// Whether `package` is listed in `dependencies` or `dev_dependencies`.
    pub fn depends_on(&self, package: &str) -> bool {
        [&self.dependencies, &self.dev_dependencies]
            .into_iter()
            .flatten()
            .any(|deps| deps.contains_key(package))
    }

    pub fn from_str(content: &str) -> Result<Self> {
        serde_yaml::from_str(content).context("Failed to parse pubspec.yaml format")
    }

    pub fn load() -> Result<Self> {
        let content = fs::read_to_string("pubspec.yaml")
            .context("Failed to read pubspec.yaml. Are you in the root of a Dart project?")?;

        Self::from_str(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_pubspec() {
        let yaml = r#"
            name: flint_example
            version: 1.0.0
        "#;
        let pubspec = Pubspec::from_str(yaml).unwrap();
        assert_eq!(pubspec.name, "flint_example");
        assert!(!pubspec.depends_on("json_serializable"));
    }

    #[test]
    fn test_depends_on() {
        let yaml = r#"
            name: app
            dependencies:
              json_annotation: ^4.9.0
            dev_dependencies:
              json_serializable:
                path: ../json_serializable
        "#;
        let pubspec = Pubspec::from_str(yaml).unwrap();
        assert!(pubspec.depends_on("json_annotation"));
        assert!(pubspec.depends_on("json_serializable"));
        assert!(!pubspec.depends_on("build_runner"));

        let empty_sections =
            Pubspec::from_str("name: app\ndependencies:\ndev_dependencies:\n").unwrap();
        assert!(!empty_sections.depends_on("json_serializable"));
    }

    #[test]
    fn test_parse_invalid_pubspec() {
        let yaml = r#"
            name: [unclosed_brackets
        "#;
        assert!(Pubspec::from_str(yaml).is_err());
    }
}
