use super::build_yaml::JsonSerializableOptions;
use anyhow::bail;
use heck::{
    ToKebabCase, ToLowerCamelCase, ToPascalCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct FlintConfig {
    pub plugins: Option<HashMap<String, PluginConfig>>,
}

#[derive(Debug, Clone, Default)]
pub struct PluginConfig {
    pub class_annotations: Vec<String>,
    pub field_annotations: Vec<String>,
    pub enum_annotations: Vec<String>,
    pub variant_annotations: Vec<String>,
    pub template_path: Option<String>,
    pub converters: Option<Vec<String>>,
    pub field_rename: Option<FieldRename>,
    /// Plugin-wide defaults for the matching `@JsonSerializable` / `@JsonKey` arguments.
    pub explicit_to_json: Option<bool>,
    pub create_factory: Option<bool>,
    pub create_to_json: Option<bool>,
    pub include_if_null: Option<bool>,
}

#[derive(Deserialize)]
struct RawPluginConfig {
    class_annotations: Option<Vec<String>>,
    field_annotations: Option<Vec<String>>,
    enum_annotations: Option<Vec<String>>,
    variant_annotations: Option<Vec<String>>,
    template_path: Option<String>,
    converters: Option<Vec<String>>,
    field_rename: Option<String>,
    explicit_to_json: Option<bool>,
    create_factory: Option<bool>,
    create_to_json: Option<bool>,
    include_if_null: Option<bool>,
}

impl<'de> serde::Deserialize<'de> for PluginConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawPluginConfig::deserialize(deserializer)?;
        Ok(PluginConfig {
            class_annotations: raw.class_annotations.unwrap_or_default(),
            field_annotations: raw.field_annotations.unwrap_or_default(),
            enum_annotations: raw.enum_annotations.unwrap_or_default(),
            variant_annotations: raw.variant_annotations.unwrap_or_default(),
            template_path: raw.template_path,
            converters: raw.converters,
            field_rename: raw
                .field_rename
                .map(|value| value.parse())
                .transpose()
                .map_err(serde::de::Error::custom)?,
            explicit_to_json: raw.explicit_to_json,
            create_factory: raw.create_factory,
            create_to_json: raw.create_to_json,
            include_if_null: raw.include_if_null,
        })
    }
}

/// How `flint_json` derives a JSON key from a field name when `@JsonKey(name:)` is absent (spec 0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRename {
    /// Keep the Dart field name, like json_serializable's `FieldRename.none`.
    None,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
    Pascal,
    /// lowerCamelCase: `user_id` → `userId`.
    Camel,
}

impl FieldRename {
    pub fn apply(self, name: &str) -> String {
        match self {
            Self::None => name.to_string(),
            Self::Snake => name.to_snake_case(),
            Self::ScreamingSnake => name.to_shouty_snake_case(),
            Self::Kebab => name.to_kebab_case(),
            Self::ScreamingKebab => name.to_shouty_kebab_case(),
            Self::Pascal => name.to_pascal_case(),
            Self::Camel => name.to_lower_camel_case(),
        }
    }
}

impl FromStr for FieldRename {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> anyhow::Result<Self> {
        Ok(match value {
            "none" => Self::None,
            "snake" | "snake_case" => Self::Snake,
            "screaming_snake" | "screaming_snake_case" => Self::ScreamingSnake,
            "kebab" | "kebab_case" => Self::Kebab,
            "screaming_kebab" | "screaming_kebab_case" => Self::ScreamingKebab,
            "pascal" | "pascal_case" => Self::Pascal,
            "camel" | "camel_case" | "lower_camel" | "lower_camel_case" => Self::Camel,
            other => bail!(
                "unknown field_rename '{other}'; expected one of: none, snake, screaming_snake, kebab, screaming_kebab, pascal, camel"
            ),
        })
    }
}

impl FlintConfig {
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let mut config: FlintConfig = serde_yaml::from_str(content)?;
        config.apply_builtin_defaults();
        Ok(config)
    }

    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// A config that only enables `flint_json` with its defaults, as if `flint.yaml` were
    /// `plugins: { flint_json: }`.
    pub fn implicit_flint_json() -> Self {
        let mut config = FlintConfig {
            plugins: Some(HashMap::from([(
                "flint_json".to_string(),
                PluginConfig::default(),
            )])),
        };
        config.apply_builtin_defaults();
        config
    }

    fn apply_builtin_defaults(&mut self) {
        let Some(plugin) = self
            .plugins
            .as_mut()
            .and_then(|plugins| plugins.get_mut("flint_json"))
        else {
            return;
        };
        if plugin.class_annotations.is_empty() {
            plugin.class_annotations = vec!["@JsonSerializable".to_string()];
        }
        if plugin.field_annotations.is_empty() {
            plugin.field_annotations = vec!["@JsonKey".to_string()];
        }
        if plugin.enum_annotations.is_empty() {
            plugin.enum_annotations = vec!["@JsonEnum".to_string()];
        }
        if plugin.variant_annotations.is_empty() {
            plugin.variant_annotations = vec!["@JsonValue".to_string()];
        }
    }
}

impl PluginConfig {
    /// Fills settings this plugin leaves unset with the json_serializable options from `build.yaml`.
    /// Returns the names of the settings that were filled.
    pub fn apply_build_yaml(&mut self, options: &JsonSerializableOptions) -> Vec<&'static str> {
        let mut applied = Vec::new();
        if self.field_rename.is_none() && options.field_rename.is_some() {
            self.field_rename = options.field_rename;
            applied.push("field_rename");
        }
        let bools = [
            (
                "explicit_to_json",
                &mut self.explicit_to_json,
                options.explicit_to_json,
            ),
            (
                "create_factory",
                &mut self.create_factory,
                options.create_factory,
            ),
            (
                "create_to_json",
                &mut self.create_to_json,
                options.create_to_json,
            ),
            (
                "include_if_null",
                &mut self.include_if_null,
                options.include_if_null,
            ),
        ];
        for (name, setting, from_build_yaml) in bools {
            if setting.is_none() && from_build_yaml.is_some() {
                *setting = from_build_yaml;
                applied.push(name);
            }
        }
        applied
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
        assert_eq!(plugin.field_rename, Some(FieldRename::Snake));
    }

    #[test]
    fn test_parse_simplified_flint_config() {
        let yaml = r#"
            plugins:
              flint_json:
        "#;
        let config = FlintConfig::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();

        assert!(plugins.contains_key("flint_json"));
        let plugin = &plugins["flint_json"];
        assert_eq!(
            plugin.class_annotations,
            vec!["@JsonSerializable".to_string()]
        );
        assert_eq!(plugin.field_annotations, vec!["@JsonKey".to_string()]);
        assert_eq!(plugin.enum_annotations, vec!["@JsonEnum".to_string()]);
        assert_eq!(plugin.variant_annotations, vec!["@JsonValue".to_string()]);
        assert_eq!(plugin.template_path, None);
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

    #[test]
    fn test_field_rename_strategies() {
        let rename =
            |strategy: &str, name: &str| strategy.parse::<FieldRename>().unwrap().apply(name);

        assert_eq!(rename("none", "myFieldName"), "myFieldName");
        assert_eq!(rename("camel", "user_id"), "userId");
        assert_eq!(rename("camel_case", "myFieldName"), "myFieldName");
        assert_eq!(rename("lower_camel", "UserId"), "userId");
        assert_eq!(rename("pascal", "myFieldName"), "MyFieldName");
        assert_eq!(rename("snake", "myFieldName"), "my_field_name");
    }

    #[test]
    fn test_unknown_field_rename_is_an_error() {
        let error = "snak".parse::<FieldRename>().unwrap_err().to_string();
        assert!(error.contains("unknown field_rename 'snak'"));

        let yaml = "plugins:\n  flint_json:\n    field_rename: snak\n";
        assert!(FlintConfig::from_str(yaml).is_err());
    }
}
