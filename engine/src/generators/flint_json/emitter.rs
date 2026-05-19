use crate::generators::{Generator, TemplateEngine};
use crate::{
    config::PluginConfig,
    parser::dart_types::{DartField, DartType, ParsedFile, TypeKind},
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
    use crate::parser::dart_types::DartType;

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
        field
            .metadata
            .insert("name".to_string(), "\"explicitName\"".to_string());
        config.field_rename = Some("snake".to_string());
        assert_eq!(extract_field_name(&mut field, &config), "explicitName");
    }
}
