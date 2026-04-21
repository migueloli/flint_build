use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Pubspec {
    pub name: String,
}

impl Pubspec {
    pub fn load() -> Result<Self> {
        let content = fs::read_to_string("pubspec.yaml")
            .context("Failed to read pubspec.yaml. Are you in the root of a Dart project?")?;

        let pubspec: Pubspec =
            serde_yaml::from_str(&content).context("Failed to parse pubspec.yaml")?;

        Ok(pubspec)
    }
}
