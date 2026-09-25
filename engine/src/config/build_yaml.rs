//! Reads json_serializable options from an existing build_runner `build.yaml`, so projects can migrate
//! without writing a `flint.yaml` (spec 0002).

use anyhow::{Result, bail};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::HashMap;

/// Keys build_runner accepts for the json_serializable builder in `targets.$default.builders`.
const BUILDER_KEYS: [&str; 3] = [
    "json_serializable",
    "json_serializable:json_serializable",
    "json_serializable|json_serializable",
];

/// Options Flint accepts without mapping them, because it already behaves that way.
const ACCEPTED_OPTIONS: [&str; 1] = ["generic_argument_factories"];

#[derive(Deserialize)]
struct BuildYaml {
    targets: Option<HashMap<String, Option<Target>>>,
}

#[derive(Deserialize)]
struct Target {
    builders: Option<HashMap<String, Option<BuilderEntry>>>,
}

#[derive(Deserialize)]
struct BuilderEntry {
    enabled: Option<bool>,
    options: Option<HashMap<String, Value>>,
}

/// The json_serializable builder's settings, already translated to Flint's names.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct JsonSerializableOptions {
    pub enabled: bool,
    pub field_rename: Option<String>,
    pub explicit_to_json: Option<bool>,
    pub create_factory: Option<bool>,
    pub create_to_json: Option<bool>,
    pub include_if_null: Option<bool>,
    /// Options Flint does not implement that were set to a non-default value.
    pub unsupported: Vec<String>,
}

/// Returns `None` when `build.yaml` does not configure the json_serializable builder.
pub fn parse(content: &str) -> Result<Option<JsonSerializableOptions>> {
    let build_yaml: Option<BuildYaml> = serde_yaml::from_str(content)?;
    let Some(builders) = build_yaml
        .and_then(|b| b.targets)
        .and_then(|mut targets| targets.remove("$default").flatten())
        .and_then(|target| target.builders)
    else {
        return Ok(None);
    };
    let Some(entry) = BUILDER_KEYS.iter().find_map(|key| builders.get(*key)) else {
        return Ok(None);
    };

    let (enabled, options) = match entry {
        Some(entry) => (
            entry.enabled.unwrap_or(true),
            entry.options.clone().unwrap_or_default(),
        ),
        None => (true, HashMap::new()),
    };

    let mut result = JsonSerializableOptions {
        enabled,
        ..Default::default()
    };
    for (name, value) in options {
        match name.as_str() {
            "field_rename" => result.field_rename = Some(field_rename(&value)?),
            "explicit_to_json" => result.explicit_to_json = Some(bool_option(&name, &value)?),
            "create_factory" => result.create_factory = Some(bool_option(&name, &value)?),
            "create_to_json" => result.create_to_json = Some(bool_option(&name, &value)?),
            "include_if_null" => result.include_if_null = Some(bool_option(&name, &value)?),
            _ if ACCEPTED_OPTIONS.contains(&name.as_str()) => {}
            _ if is_default_value(&value) => {}
            _ => result.unsupported.push(name),
        }
    }
    result.unsupported.sort();
    Ok(Some(result))
}

fn bool_option(name: &str, value: &Value) -> Result<bool> {
    match value.as_bool() {
        Some(b) => Ok(b),
        None => bail!("build.yaml: json_serializable option '{name}' must be true or false"),
    }
}

/// Maps json_serializable's `FieldRename` names to Flint's `field_rename` values.
fn field_rename(value: &Value) -> Result<String> {
    let flint_name = match value.as_str() {
        Some("none") => "none",
        Some("kebab") => "kebab",
        Some("snake") => "snake",
        Some("pascal") => "pascal",
        Some("screamingSnake") => "screaming_snake",
        _ => bail!(
            "build.yaml: json_serializable option 'field_rename' must be one of none, kebab, snake, pascal, screamingSnake"
        ),
    };
    Ok(flint_name.to_string())
}

/// Every json_serializable option Flint does not implement defaults to `false` or `""`.
fn is_default_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_supported_options() {
        let yaml = r#"
            targets:
              $default:
                builders:
                  json_serializable:
                    options:
                      field_rename: screamingSnake
                      explicit_to_json: true
                      create_factory: false
                      create_to_json: true
                      include_if_null: false
                      generic_argument_factories: true
        "#;
        let options = parse(yaml).unwrap().unwrap();
        assert_eq!(
            options,
            JsonSerializableOptions {
                enabled: true,
                field_rename: Some("screaming_snake".to_string()),
                explicit_to_json: Some(true),
                create_factory: Some(false),
                create_to_json: Some(true),
                include_if_null: Some(false),
                unsupported: vec![],
            }
        );
    }

    #[test]
    fn test_parse_fully_qualified_builder_key_and_enabled() {
        let yaml = r#"
            targets:
              $default:
                builders:
                  json_serializable:json_serializable:
                    enabled: false
        "#;
        let options = parse(yaml).unwrap().unwrap();
        assert!(!options.enabled);
        assert_eq!(options.field_rename, None);
    }

    #[test]
    fn test_parse_builder_without_options() {
        let yaml = r#"
            targets:
              $default:
                builders:
                  json_serializable:
        "#;
        let options = parse(yaml).unwrap().unwrap();
        assert!(options.enabled);
        assert!(options.unsupported.is_empty());
    }

    #[test]
    fn test_parse_without_json_serializable() {
        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse("targets:\n").unwrap(), None);
        let yaml = r#"
            targets:
              $default:
                builders:
                  freezed:
                    options:
                      copy_with: false
        "#;
        assert_eq!(parse(yaml).unwrap(), None);
    }

    #[test]
    fn test_parse_reports_unsupported_options_only_when_set() {
        let yaml = r#"
            targets:
              $default:
                builders:
                  json_serializable:
                    options:
                      any_map: true
                      checked: false
                      constructor: ""
                      disallow_unrecognized_keys: true
        "#;
        let options = parse(yaml).unwrap().unwrap();
        assert_eq!(
            options.unsupported,
            vec![
                "any_map".to_string(),
                "disallow_unrecognized_keys".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_rejects_invalid_values() {
        let wrong_type = r#"
            targets:
              $default:
                builders:
                  json_serializable:
                    options:
                      explicit_to_json: "yes"
        "#;
        assert!(parse(wrong_type).is_err());

        let unknown_rename = r#"
            targets:
              $default:
                builders:
                  json_serializable:
                    options:
                      field_rename: camel
        "#;
        assert!(parse(unknown_rename).is_err());
    }
}
