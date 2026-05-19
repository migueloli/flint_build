use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Pubspec {
    pub name: String,
}

impl Pubspec {
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
    }

    #[test]
    fn test_parse_invalid_pubspec() {
        let yaml = r#"
            name: [unclosed_brackets
        "#;
        assert!(Pubspec::from_str(yaml).is_err());
    }
}
