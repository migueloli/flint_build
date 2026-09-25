use crate::generators::{Generator, TemplateEngine};
use crate::{
    config::PluginConfig,
    parser::dart_types::{DartClass, DartField, DartType, ParsedFile, TypeKind},
};
use heck::{
    ToKebabCase, ToLowerCamelCase, ToPascalCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
    ToUpperCamelCase,
};
use tera::Context;

pub struct FlintJsonGenerator;

impl Generator for FlintJsonGenerator {
    fn generate(&self, filename: &str, parsed_file: ParsedFile, plugin: &PluginConfig) -> String {
        generate_full_file(filename, parsed_file, plugin)
    }
}

pub fn generate_full_file(
    filename: &str,
    mut parsed_file: ParsedFile,
    plugin: &PluginConfig,
) -> String {
    parsed_file.classes.retain(|class| {
        class
            .metadata
            .keys()
            .any(|k| plugin.class_annotations.contains(&format!("@{}", k)))
    });

    parsed_file.enums.retain(|e| {
        e.annotations.iter().any(|a| {
            plugin
                .enum_annotations
                .contains(&format!("@{}", a.trim_start_matches('@')))
        })
    });

    let enum_names: Vec<String> = parsed_file.enums.iter().map(|e| e.name.clone()).collect();

    for class in &mut parsed_file.classes {
        log::debug!(
            "Generating code for class: {} ({} fields)",
            class.name,
            class.fields.len()
        );

        apply_plugin_defaults(class, plugin);
        let explicit_to_json =
            class.metadata.get("explicitToJson").map(|v| v.as_str()) == Some("true");
        for field in &mut class.fields {
            if let Some(converters) = &plugin.converters {
                for key in field.metadata.keys() {
                    let full_annotation = format!("@{}", key);
                    if converters.contains(&full_annotation) {
                        field.converter = Some(key.clone());
                        break;
                    }
                }
            }

            let key = extract_field_name(field, plugin);
            let from_access = format!("json['{}']", key);
            let to_access = format!("instance.{}", field.name);
            if let Some(converter) = &field.converter {
                field.from_json_expr =
                    Some(format!("const {}().fromJson({})", converter, from_access));
                field.to_json_expr = Some(format!("const {}().toJson({})", converter, to_access));
            } else {
                field.from_json_expr = Some(generate_from_json_expression(
                    &field.dart_type,
                    &from_access,
                    &enum_names,
                    &class.type_parameters,
                ));
                field.to_json_expr = Some(generate_to_json_expression(
                    &field.dart_type,
                    &to_access,
                    explicit_to_json,
                    &enum_names,
                    &class.type_parameters,
                ));
            }
        }
    }

    let mut engine = TemplateEngine::new();
    let internal_template = include_str!("../../templates/flint_json.tera");
    engine.load_template(
        "flint_json",
        internal_template,
        plugin.template_path.as_ref(),
    );

    let mut context = Context::new();
    context.insert("classes", &parsed_file.classes);
    context.insert("enums", &parsed_file.enums);
    context.insert("filename", filename);

    engine.render("flint_json", &context)
}

/// Fills options the class's annotations leave out with the plugin-wide defaults from `flint.yaml` or
/// `build.yaml`, so the template only has to read metadata. Class- and plugin-level `includeIfNull`
/// only reaches nullable fields, as in json_serializable.
fn apply_plugin_defaults(class: &mut DartClass, plugin: &PluginConfig) {
    let defaults = [
        ("explicitToJson", plugin.explicit_to_json),
        ("createFactory", plugin.create_factory),
        ("createToJson", plugin.create_to_json),
        ("includeIfNull", plugin.include_if_null),
    ];
    for (key, value) in defaults {
        if let Some(value) = value {
            class
                .metadata
                .entry(key.to_string())
                .or_insert_with(|| value.to_string());
        }
    }

    if let Some(include_if_null) = class.metadata.get("includeIfNull") {
        for field in class.fields.iter_mut().filter(|f| f.dart_type.is_nullable) {
            field
                .metadata
                .entry("includeIfNull".to_string())
                .or_insert_with(|| include_if_null.clone());
        }
    }
}

fn generate_from_json_expression(
    dart_type: &DartType,
    access: &str,
    enum_names: &[String],
    type_params: &[String],
) -> String {
    let expression = match &dart_type.kind {
        TypeKind::String => format!(
            "{} as String{}",
            access,
            if dart_type.is_nullable { "?" } else { "" }
        ),
        TypeKind::Bool => format!("{} as bool", access),
        TypeKind::Int => format!("({} as num).toInt()", access),
        TypeKind::Double => format!("({} as num).toDouble()", access),
        TypeKind::DateTime => format!("DateTime.parse({} as String)", access),
        TypeKind::List(inner) => {
            let element = "e";
            let inner_expr = generate_from_json_expression(inner, element, enum_names, type_params);
            format!(
                "({} as List<dynamic>).map(({}) => {}).toList()",
                access, element, inner_expr
            )
        }
        TypeKind::Map(k, v) => {
            let key = "k";
            let value = "v";
            let key_expr = generate_from_json_expression(k, key, enum_names, type_params);
            let value_expr = generate_from_json_expression(v, value, enum_names, type_params);
            format!(
                "({} as Map<String, dynamic>).map(({}, {}) => MapEntry({}, {}))",
                access, key, value, key_expr, value_expr
            )
        }
        TypeKind::Custom(name) => {
            if enum_names.contains(&name.to_string()) {
                format!(
                    "_${}EnumMap.entries.firstWhere((e) => e.value == {}).key",
                    name, access
                )
            } else if type_params.contains(&name.to_string()) {
                format!("fromJson{}({} as Object?)", name, access)
            } else {
                format!("{}.fromJson({} as Map<String, dynamic>)", name, access)
            }
        }
    };

    if dart_type.is_nullable && !matches!(dart_type.kind, TypeKind::String) {
        format!("{} == null ? null : {}", access, expression)
    } else {
        expression
    }
}

fn generate_to_json_expression(
    dart_type: &DartType,
    access: &str,
    explicit_to_json: bool,
    enum_names: &[String],
    type_params: &[String],
) -> String {
    match &dart_type.kind {
        TypeKind::DateTime => {
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}toIso8601String()", access, op)
        }
        TypeKind::Custom(name) => {
            if enum_names.contains(&name.to_string()) {
                format!("_${}EnumMap[{}]", name, access)
            } else if type_params.contains(&name.to_string()) {
                format!("toJson{}({})", name, access)
            } else {
                let op = if dart_type.is_nullable { "?." } else { "." };
                if explicit_to_json {
                    format!("{}{}toJson()", access, op)
                } else {
                    access.to_string()
                }
            }
        }
        TypeKind::List(inner) => {
            let inner_expr = generate_to_json_expression(
                inner,
                "elem",
                explicit_to_json,
                enum_names,
                type_params,
            );
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}map((elem) => {}).toList()", access, op, inner_expr)
        }
        TypeKind::Map(k, v) => {
            let key_expr =
                generate_to_json_expression(k, "key", explicit_to_json, enum_names, type_params);
            let value_expr =
                generate_to_json_expression(v, "value", explicit_to_json, enum_names, type_params);
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!(
                "{}{}map((key, value) => MapEntry({}, {}))",
                access, op, key_expr, value_expr
            )
        }
        _ => access.to_string(),
    }
}

fn extract_field_name(field: &mut DartField, plugin: &PluginConfig) -> String {
    if let Some(raw_key) = field.metadata.get("name") {
        let clean_key = raw_key.trim_matches(|c| c == '"' || c == '\'').to_string();
        field.metadata.insert("name".to_string(), clean_key.clone());
        clean_key
    } else if let Some(strategy) = &plugin.field_rename {
        let renamed = match strategy.as_str() {
            "snake" | "snake_case" => field.name.to_snake_case(),
            "screaming_snake" | "screaming_snake_case" => field.name.to_shouty_snake_case(),
            "kebab" | "kebab_case" => field.name.to_kebab_case(),
            "screaming_kebab" | "screaming_kebab_case" => field.name.to_shouty_kebab_case(),
            "pascal" | "pascal_case" => field.name.to_pascal_case(),
            "camel" | "camel_case" => field.name.to_upper_camel_case(),
            "lower_camel" | "lower_camel_case" => field.name.to_lower_camel_case(),
            _ => field.name.clone(),
        };
        field.metadata.insert("name".to_string(), renamed.clone());
        renamed
    } else {
        field
            .metadata
            .insert("name".to_string(), field.name.clone());
        field.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dart_types::{DartClass, DartType, ParsedFile};

    #[test]
    fn test_extract_field_name_casing() {
        let make_field = |name: &str| DartField {
            name: name.to_string(),
            dart_type: DartType {
                kind: TypeKind::String,
                is_nullable: false,
            },
            is_final: true,
            from_json_expr: None,
            to_json_expr: None,
            metadata: std::collections::HashMap::new(),
            converter: None,
        };

        let mut field = make_field("myCamelCaseField");
        let mut config = PluginConfig {
            class_annotations: vec![],
            enum_annotations: vec![],
            field_annotations: vec![],
            variant_annotations: vec![],
            field_rename: Some("snake_case".to_string()),
            converters: None,
            template_path: None,
            ..Default::default()
        };
        assert_eq!(
            extract_field_name(&mut field, &config),
            "my_camel_case_field"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_snake".to_string());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY_CAMEL_CASE_FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("kebab".to_string());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "my-camel-case-field"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("pascal".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("pascal_case".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("camel".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("camel_case".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_kebab".to_string());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY-CAMEL-CASE-FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_kebab_case".to_string());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY-CAMEL-CASE-FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("lower_camel".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("lower_camel_case".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("myCamelCaseField");
        field
            .metadata
            .insert("name".to_string(), "\"explicitName\"".to_string());
        config.field_rename = Some("snake".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "explicitName");
    }

    #[test]
    fn test_custom_converters() {
        let field = DartField {
            name: "createdAt".to_string(),
            dart_type: DartType {
                kind: TypeKind::Custom("DateTime".to_string()),
                is_nullable: false,
            },
            is_final: true,
            from_json_expr: None,
            to_json_expr: None,
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("MyDateTimeConverter".to_string(), "".to_string());
                m
            },
            converter: None,
        };

        let class = DartClass {
            name: "User".to_string(),
            fields: vec![field],
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("JsonSerializable".to_string(), "".to_string());
                m
            },
            type_parameters: vec![],
        };

        let parsed_file = ParsedFile {
            classes: vec![class],
            enums: vec![],
        };

        let output = generate_full_file(
            "user.dart",
            parsed_file,
            &PluginConfig {
                class_annotations: vec!["@JsonSerializable".to_string()],
                enum_annotations: vec![],
                field_annotations: vec![],
                variant_annotations: vec![],
                field_rename: None,
                converters: Some(vec!["@MyDateTimeConverter".to_string()]),
                template_path: None,
                ..Default::default()
            },
        );

        assert!(output.contains("const MyDateTimeConverter().fromJson"));
    }

    #[test]
    fn test_explicit_to_json() {
        let field = DartField {
            name: "address".to_string(),
            dart_type: DartType {
                kind: TypeKind::Custom("Address".to_string()),
                is_nullable: true,
            },
            is_final: true,
            from_json_expr: None,
            to_json_expr: None,
            metadata: std::collections::HashMap::new(),
            converter: None,
        };

        let class = DartClass {
            name: "User".to_string(),
            fields: vec![field],
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("JsonSerializable".to_string(), "".to_string());
                m.insert("explicitToJson".to_string(), "true".to_string());
                m
            },
            type_parameters: vec![],
        };

        let parsed_file = ParsedFile {
            classes: vec![class],
            enums: vec![],
        };

        let output = generate_full_file(
            "user.dart",
            parsed_file,
            &PluginConfig {
                class_annotations: vec!["@JsonSerializable".to_string()],
                enum_annotations: vec![],
                field_annotations: vec![],
                variant_annotations: vec![],
                field_rename: None,
                converters: None,
                template_path: None,
                ..Default::default()
            },
        );

        assert!(output.contains("address?.toJson()"));
    }

    fn field(name: &str, kind: TypeKind, is_nullable: bool) -> DartField {
        DartField {
            name: name.to_string(),
            dart_type: DartType { kind, is_nullable },
            is_final: true,
            from_json_expr: None,
            to_json_expr: None,
            metadata: std::collections::HashMap::new(),
            converter: None,
        }
    }

    fn user_file(class_metadata: &[(&str, &str)], fields: Vec<DartField>) -> ParsedFile {
        let mut metadata =
            std::collections::HashMap::from([("JsonSerializable".to_string(), String::new())]);
        for (key, value) in class_metadata {
            metadata.insert(key.to_string(), value.to_string());
        }
        ParsedFile {
            classes: vec![DartClass {
                name: "User".to_string(),
                fields,
                metadata,
                type_parameters: vec![],
            }],
            enums: vec![],
        }
    }

    #[test]
    fn test_plugin_defaults_apply_unless_annotation_overrides() {
        let plugin = PluginConfig {
            class_annotations: vec!["@JsonSerializable".to_string()],
            explicit_to_json: Some(true),
            create_factory: Some(false),
            ..Default::default()
        };
        let address = || field("address", TypeKind::Custom("Address".to_string()), false);

        let output = generate_full_file("user.dart", user_file(&[], vec![address()]), &plugin);
        assert!(output.contains("'address': instance.address.toJson(),"));
        assert!(!output.contains("_$UserFromJson"));

        let overridden = user_file(
            &[("explicitToJson", "false"), ("createFactory", "true")],
            vec![address()],
        );
        let output = generate_full_file("user.dart", overridden, &plugin);
        assert!(output.contains("'address': instance.address,"));
        assert!(output.contains("_$UserFromJson"));
    }

    #[test]
    fn test_include_if_null_default_only_applies_to_nullable_fields() {
        let fields = || {
            vec![
                field("id", TypeKind::Int, false),
                field("nickname", TypeKind::String, true),
            ]
        };
        let base = PluginConfig {
            class_annotations: vec!["@JsonSerializable".to_string()],
            ..Default::default()
        };

        let from_plugin = PluginConfig {
            include_if_null: Some(false),
            ..base.clone()
        };
        let from_class = user_file(&[("includeIfNull", "false")], fields());

        for output in [
            generate_full_file("user.dart", user_file(&[], fields()), &from_plugin),
            generate_full_file("user.dart", from_class, &base),
        ] {
            assert!(output.contains("if (instance.nickname != null)"));
            assert!(!output.contains("if (instance.id != null)"));
        }

        let output = generate_full_file("user.dart", user_file(&[], fields()), &base);
        assert!(!output.contains("!= null)"));
    }
}
