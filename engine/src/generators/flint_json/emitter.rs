use crate::error::FlintError;
use crate::generators::{Generator, TemplateEngine, retain_annotated, select_variant_values};
use crate::{
    config::PluginConfig,
    parser::dart_types::{DartClass, DartField, DartType, ParsedFile, TypeKind},
};
use tera::Context;

pub struct FlintJsonGenerator;

impl Generator for FlintJsonGenerator {
    fn generate(
        &self,
        filename: &str,
        parsed_file: ParsedFile,
        plugin: &PluginConfig,
    ) -> Result<String, FlintError> {
        generate_full_file(filename, parsed_file, plugin)
    }
}

pub fn generate_full_file(
    filename: &str,
    mut parsed_file: ParsedFile,
    plugin: &PluginConfig,
) -> Result<String, FlintError> {
    retain_annotated(&mut parsed_file, plugin);
    select_variant_values(&mut parsed_file, plugin);
    for value in parsed_file
        .enums
        .iter_mut()
        .flat_map(|e| e.values.iter_mut())
    {
        value.literal = value.literal.take().map(prefer_single_quotes);
    }

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
        let creates_factory =
            class.metadata.get("createFactory").map(|v| v.as_str()) != Some("false");
        let creates_to_json =
            class.metadata.get("createToJson").map(|v| v.as_str()) != Some("false");
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

            if field.converter.is_none()
                && contains_unsupported(&field.dart_type)
                && needs_generated_conversion(field, creates_factory, creates_to_json)
            {
                let problem = match &field.dart_type.kind {
                    TypeKind::Unsupported(text) if text.is_empty() => {
                        "has no declared type".to_string()
                    }
                    _ => format!(
                        "has type '{}', which Flint can't serialize yet",
                        field.dart_type
                    ),
                };
                return Err(FlintError::UnsupportedType {
                    line: field.line,
                    class: class.name.clone(),
                    field: field.name.clone(),
                    problem,
                });
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

    let template_error = |e: tera::Error| FlintError::template("flint_json", &e);
    let mut engine = TemplateEngine::new();
    let internal_template = include_str!("../../templates/flint_json.tera");
    engine
        .load_template(
            "flint_json",
            internal_template,
            plugin.template_path.as_ref(),
        )
        .map_err(template_error)?;

    let mut context = Context::new();
    context.insert("classes", &parsed_file.classes);
    context.insert("enums", &parsed_file.enums);
    context.insert("filename", filename);

    engine
        .render("flint_json", &context)
        .map_err(template_error)
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

/// Rewrites a simple double-quoted string literal with single quotes, as json_serializable writes them
/// (`"active"` → `'active'`). Anything that could change meaning (`'`, `\`, raw or triple quotes) or
/// isn't a string (`1`, `true`) is kept exactly as written.
fn prefer_single_quotes(literal: String) -> String {
    match literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inner) if !inner.contains(['\'', '"', '\\']) && !literal.starts_with("\"\"\"") => {
            format!("'{inner}'")
        }
        _ => literal,
    }
}

/// Whether a record, function type or missing type appears anywhere in the type (spec 0005).
fn contains_unsupported(dart_type: &DartType) -> bool {
    match &dart_type.kind {
        TypeKind::Unsupported(_) => true,
        TypeKind::List(inner) => contains_unsupported(inner),
        TypeKind::Map(key, value) => contains_unsupported(key) || contains_unsupported(value),
        _ => false,
    }
}

/// Whether the template will emit a generated conversion for this field, on either side. Fields that are
/// ignored, excluded, or have their own `@JsonKey(fromJson:/toJson:)` hooks don't need one.
fn needs_generated_conversion(
    field: &DartField,
    creates_factory: bool,
    creates_to_json: bool,
) -> bool {
    let is = |key: &str, value: &str| field.metadata.get(key).map(String::as_str) == Some(value);
    if is("ignore", "true") {
        return false;
    }
    let from = creates_factory
        && !is("includeFromJson", "false")
        && !field.metadata.contains_key("fromJson");
    let to =
        creates_to_json && !is("includeToJson", "false") && !field.metadata.contains_key("toJson");
    from || to
}

/// The enum's name if a map key has an enum type generated in this file.
fn enum_key<'a>(key: &'a DartType, enum_names: &[String]) -> Option<&'a str> {
    match &key.kind {
        TypeKind::Custom(name) if enum_names.contains(name) => Some(name),
        _ => None,
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
            // JSON object keys are always strings, so an enum key is matched by its value's string form
            // (`@JsonValue(1)` is the key "1").
            let key_expr = match enum_key(k, enum_names) {
                Some(name) => format!(
                    "_${name}EnumMap.entries.firstWhere((e) => e.value.toString() == {key}).key"
                ),
                None => generate_from_json_expression(k, key, enum_names, type_params),
            };
            let value_expr = generate_from_json_expression(v, value, enum_names, type_params);
            format!(
                "({} as Map<String, dynamic>).map(({}, {}) => MapEntry({}, {}))",
                access, key, value, key_expr, value_expr
            )
        }
        // Only reached when the template won't use the expression (see needs_generated_conversion).
        TypeKind::Unsupported(_) => access.to_string(),
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
            let key_expr = match enum_key(k, enum_names) {
                Some(name) => format!("_${name}EnumMap[key].toString()"),
                None => {
                    generate_to_json_expression(k, "key", explicit_to_json, enum_names, type_params)
                }
            };
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
    } else if let Some(strategy) = plugin.field_rename {
        let renamed = strategy.apply(&field.name);
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
    fn test_prefer_single_quotes() {
        let q = |s: &str| prefer_single_quotes(s.to_string());
        assert_eq!(q("\"active\""), "'active'");
        assert_eq!(q("'active'"), "'active'");
        assert_eq!(q("\"it's\""), "\"it's\"");
        assert_eq!(q("\"a\\nb\""), "\"a\\nb\"");
        assert_eq!(q("\"\"\"doc\"\"\""), "\"\"\"doc\"\"\"");
        assert_eq!(q("r\"raw\""), "r\"raw\"");
        assert_eq!(q("1"), "1");
        assert_eq!(q("\"\""), "''");
    }

    #[test]
    fn test_extract_field_name_casing() {
        let make_field = |name: &str| DartField {
            name: name.to_string(),
            line: 1,
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
            field_rename: Some("snake_case".parse().unwrap()),
            converters: None,
            template_path: None,
            ..Default::default()
        };
        assert_eq!(
            extract_field_name(&mut field, &config),
            "my_camel_case_field"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_snake".parse().unwrap());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY_CAMEL_CASE_FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("kebab".parse().unwrap());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "my-camel-case-field"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("pascal".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("pascal_case".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "MyCamelCaseField");

        field = make_field("my_camel_case_field");
        config.field_rename = Some("camel".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("my_camel_case_field");
        config.field_rename = Some("camel_case".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_kebab".parse().unwrap());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY-CAMEL-CASE-FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("screaming_kebab_case".parse().unwrap());
        assert_eq!(
            extract_field_name(&mut field, &config),
            "MY-CAMEL-CASE-FIELD"
        );

        field = make_field("myCamelCaseField");
        config.field_rename = Some("lower_camel".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("myCamelCaseField");
        config.field_rename = Some("lower_camel_case".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "myCamelCaseField");

        field = make_field("myCamelCaseField");
        field
            .metadata
            .insert("name".to_string(), "\"explicitName\"".to_string());
        config.field_rename = Some("snake".parse().unwrap());
        assert_eq!(extract_field_name(&mut field, &config), "explicitName");
    }

    #[test]
    fn test_custom_converters() {
        let field = DartField {
            name: "createdAt".to_string(),
            line: 1,
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
            ..Default::default()
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
        )
        .unwrap();

        assert!(output.contains("const MyDateTimeConverter().fromJson"));
    }

    #[test]
    fn test_explicit_to_json() {
        let field = DartField {
            name: "address".to_string(),
            line: 1,
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
            ..Default::default()
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
        )
        .unwrap();

        assert!(output.contains("address?.toJson()"));
    }

    fn field(name: &str, kind: TypeKind, is_nullable: bool) -> DartField {
        DartField {
            name: name.to_string(),
            line: 1,
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
            ..Default::default()
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

        let output =
            generate_full_file("user.dart", user_file(&[], vec![address()]), &plugin).unwrap();
        assert!(output.contains("'address': instance.address.toJson(),"));
        assert!(!output.contains("_$UserFromJson"));

        let overridden = user_file(
            &[("explicitToJson", "false"), ("createFactory", "true")],
            vec![address()],
        );
        let output = generate_full_file("user.dart", overridden, &plugin).unwrap();
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
            generate_full_file("user.dart", user_file(&[], fields()), &from_plugin).unwrap(),
            generate_full_file("user.dart", from_class, &base).unwrap(),
        ] {
            assert!(output.contains("if (instance.nickname != null)"));
            assert!(!output.contains("if (instance.id != null)"));
        }

        let output = generate_full_file("user.dart", user_file(&[], fields()), &base).unwrap();
        assert!(!output.contains("!= null)"));
    }

    #[test]
    fn test_unsupported_type_is_an_error_only_when_a_conversion_is_generated() {
        let plugin = PluginConfig {
            class_annotations: vec!["@JsonSerializable".to_string()],
            ..Default::default()
        };
        let record = || {
            field(
                "pair",
                TypeKind::Unsupported("(int, String)".to_string()),
                false,
            )
        };

        let error = generate_full_file("user.dart", user_file(&[], vec![record()]), &plugin)
            .unwrap_err()
            .to_string();
        assert!(error.contains("field 'pair' of 'User'"), "{error}");
        assert!(error.contains("'(int, String)'"), "{error}");

        let mut ignored = record();
        ignored
            .metadata
            .insert("ignore".to_string(), "true".to_string());
        let mut hooked = record();
        hooked
            .metadata
            .insert("fromJson".to_string(), "_pairFromJson".to_string());
        hooked
            .metadata
            .insert("toJson".to_string(), "_pairToJson".to_string());
        for field in [ignored, hooked] {
            assert!(generate_full_file("user.dart", user_file(&[], vec![field]), &plugin).is_ok());
        }

        let untyped = field("guess", TypeKind::Unsupported(String::new()), false);
        let error = generate_full_file("user.dart", user_file(&[], vec![untyped]), &plugin)
            .unwrap_err()
            .to_string();
        assert!(error.contains("has no declared type"), "{error}");
    }
}
