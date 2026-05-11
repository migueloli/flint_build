use crate::parser::dart_types::{DartType, ParsedFile, TypeKind};
use tera::{Context, Tera};

pub fn generate_full_file(
    filename: &str,
    mut parsed_file: ParsedFile,
    template_path: &str,
) -> String {
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
            if let Some(raw_key) = field.metadata.get("name") {
                let key = raw_key.trim_matches(|c| c == '"' || c == '\'').to_string();
                field.metadata.insert("name".to_string(), key);
            }
            let key = field.metadata.get("name").unwrap_or(&field.name);
            let from_access = format!("json['{}']", key);
            let to_access = format!("instance.{}", field.name);
            field.from_json_expr = Some(generate_from_json_expression(
                &field.dart_type,
                &from_access,
                &enum_names,
            ));
            field.to_json_expr = Some(generate_to_json_expression(
                &field.dart_type,
                &to_access,
                explicit_to_json,
                &enum_names,
            ));
        }
    }
    let mut tera = Tera::default();
    tera.add_template_file(template_path, Some("flint_json"))
        .expect("Failed to load template");

    let mut context = Context::new();
    context.insert("classes", &parsed_file.classes);
    context.insert("enums", &parsed_file.enums);
    context.insert("filename", filename);

    let rendered = tera.render("flint_json", &context).unwrap();
    rendered
}

fn generate_from_json_expression(
    dart_type: &DartType,
    access: &str,
    enum_names: &[String],
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
            let inner_expr = generate_from_json_expression(inner, element, enum_names);
            format!(
                "({} as List<dynamic>).map(({}) => {}).toList()",
                access, element, inner_expr
            )
        }
        TypeKind::Map(k, v) => {
            let key = "k";
            let value = "v";
            let key_expr = generate_from_json_expression(k, key, enum_names);
            let value_expr = generate_from_json_expression(v, value, enum_names);
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
) -> String {
    match &dart_type.kind {
        TypeKind::DateTime => {
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}toIso8601String()", access, op)
        }
        TypeKind::Custom(name) => {
            if enum_names.contains(&name.to_string()) {
                format!("_${}EnumMap[{}]", name, access)
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
            let inner_expr =
                generate_to_json_expression(inner, "elem", explicit_to_json, enum_names);
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}map((elem) => {}).toList()", access, op, inner_expr)
        }
        TypeKind::Map(k, v) => {
            let key_expr = generate_to_json_expression(k, "key", explicit_to_json, enum_names);
            let value_expr = generate_to_json_expression(v, "value", explicit_to_json, enum_names);
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!(
                "{}{}map((key, value) => MapEntry({}, {}))",
                access, op, key_expr, value_expr
            )
        }
        _ => access.to_string(),
    }
}
