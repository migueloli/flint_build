pub mod pubspec;
pub use pubspec::Pubspec;

pub mod build_yaml;
pub mod flint;
pub use flint::*;

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// The configuration a build runs with, plus messages about where it came from.
#[derive(Debug)]
pub struct ProjectConfig {
    pub flint: FlintConfig,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

/// Reads `flint.yaml` and `build.yaml` from `root` (either may be missing) and resolves them.
pub fn load_project_config(root: &Path, pubspec: &Pubspec) -> Result<ProjectConfig> {
    let read_optional = |name: &str| -> Result<Option<String>> {
        let path = root.join(name);
        if !path.exists() {
            return Ok(None);
        }
        fs::read_to_string(&path)
            .map(Some)
            .with_context(|| format!("Failed to read {}", path.display()))
    };
    resolve(
        read_optional("flint.yaml")?.as_deref(),
        read_optional("build.yaml")?.as_deref(),
        pubspec,
    )
}

/// Combines `flint.yaml`, `build.yaml` and `pubspec.yaml` into one config (spec 0002).
///
/// `flint.yaml` wins over `build.yaml`. Without a `flint.yaml`, `flint_json` is enabled when the project
/// uses json_serializable.
pub fn resolve(
    flint_yaml: Option<&str>,
    build_yaml: Option<&str>,
    pubspec: &Pubspec,
) -> Result<ProjectConfig> {
    let json_options = build_yaml
        .map(build_yaml::parse)
        .transpose()
        .context("Failed to parse build.yaml")?
        .flatten();

    let mut notes = Vec::new();
    let mut warnings = Vec::new();

    let mut flint = match flint_yaml {
        Some(content) => FlintConfig::from_str(content).context("Failed to parse flint.yaml")?,
        None => {
            let (enabled, reason) = match &json_options {
                Some(options) => (
                    options.enabled,
                    "json_serializable is configured in build.yaml",
                ),
                None => (
                    pubspec.depends_on("json_serializable"),
                    "pubspec.yaml depends on json_serializable",
                ),
            };
            if !enabled {
                let why = if json_options.is_some() {
                    "the json_serializable builder is disabled in build.yaml"
                } else {
                    "this project doesn't use json_serializable"
                };
                bail!(
                    "No flint.yaml found, and {why}. Create a flint.yaml to configure your plugins."
                );
            }
            notes.push(format!(
                "No flint.yaml found; enabling flint_json because {reason}."
            ));
            FlintConfig::implicit_flint_json()
        }
    };

    if let Some(options) = &json_options
        && let Some(plugin) = flint
            .plugins
            .as_mut()
            .and_then(|plugins| plugins.get_mut("flint_json"))
    {
        let applied = plugin.apply_build_yaml(options);
        if !applied.is_empty() {
            notes.push(format!(
                "Using json_serializable options from build.yaml: {}.",
                applied.join(", ")
            ));
        }
        warnings.extend(options.unsupported.iter().map(|name| {
            format!("build.yaml: json_serializable option '{name}' is not supported by Flint and was ignored.")
        }));
    }

    Ok(ProjectConfig {
        flint,
        notes,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD_YAML: &str = r#"
        targets:
          $default:
            builders:
              json_serializable:
                options:
                  field_rename: snake
                  explicit_to_json: true
                  any_map: true
    "#;

    fn pubspec(with_json_serializable: bool) -> Pubspec {
        let deps = if with_json_serializable {
            "dev_dependencies:\n  json_serializable: ^6.0.0\n"
        } else {
            ""
        };
        Pubspec::from_str(&format!("name: app\n{deps}")).unwrap()
    }

    fn flint_json(config: &ProjectConfig) -> &PluginConfig {
        &config.flint.plugins.as_ref().unwrap()["flint_json"]
    }

    #[test]
    fn test_no_config_files_uses_pubspec_dependency() {
        let config = resolve(None, None, &pubspec(true)).unwrap();
        assert_eq!(
            flint_json(&config).class_annotations,
            vec!["@JsonSerializable".to_string()]
        );
        assert!(config.notes[0].contains("pubspec.yaml"));

        assert!(resolve(None, None, &pubspec(false)).is_err());
    }

    #[test]
    fn test_build_yaml_alone_enables_and_configures_flint_json() {
        let config = resolve(None, Some(BUILD_YAML), &pubspec(false)).unwrap();
        let plugin = flint_json(&config);
        assert_eq!(plugin.field_rename, Some("snake".to_string()));
        assert_eq!(plugin.explicit_to_json, Some(true));
        assert_eq!(config.notes.len(), 2);
        assert_eq!(config.warnings.len(), 1);
        assert!(config.warnings[0].contains("any_map"));
    }

    #[test]
    fn test_build_yaml_disabled_builder_is_respected() {
        let build_yaml = r#"
            targets:
              $default:
                builders:
                  json_serializable:
                    enabled: false
        "#;
        let error = resolve(None, Some(build_yaml), &pubspec(true)).unwrap_err();
        assert!(error.to_string().contains("disabled in build.yaml"));
    }

    #[test]
    fn test_flint_yaml_overrides_build_yaml() {
        let flint_yaml = "plugins:\n  flint_json:\n    field_rename: kebab\n";
        let config = resolve(Some(flint_yaml), Some(BUILD_YAML), &pubspec(true)).unwrap();
        let plugin = flint_json(&config);
        assert_eq!(plugin.field_rename, Some("kebab".to_string()));
        assert_eq!(plugin.explicit_to_json, Some(true));
        assert_eq!(
            config.notes,
            vec!["Using json_serializable options from build.yaml: explicit_to_json.".to_string()]
        );
    }

    #[test]
    fn test_flint_yaml_without_flint_json_ignores_build_yaml() {
        let flint_yaml = "plugins:\n  custom:\n    template_path: t.tera\n";
        let config = resolve(Some(flint_yaml), Some(BUILD_YAML), &pubspec(true)).unwrap();
        assert!(
            !config
                .flint
                .plugins
                .as_ref()
                .unwrap()
                .contains_key("flint_json")
        );
        assert!(config.notes.is_empty() && config.warnings.is_empty());
    }

    #[test]
    fn test_invalid_build_yaml_is_an_error() {
        assert!(resolve(None, Some("targets: [oops"), &pubspec(true)).is_err());
    }
}
