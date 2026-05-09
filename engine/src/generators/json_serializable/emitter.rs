use crate::parser::dart_types::{DartClass, DartType, TypeKind};
use tera::{Context, Tera};

pub fn generate_full_file(filename: &str, mut classes: Vec<DartClass>) -> String {
    for class in &mut classes {
        for field in &mut class.fields {
            let key = field.metadata.get("name").unwrap_or(&field.name);
            let from_access = format!("json['{}']", key);
            let to_access = format!("instance.{}", field.name);
            field.from_json_expr = Some(generate_from_json_expression(
                &field.dart_type,
                &from_access,
            ));
            field.to_json_expr = Some(generate_to_json_expression(&field.dart_type, &to_access));
        }
    }
    let mut tera = Tera::default();
    tera.add_template_file(
        "src/templates/json_serializable.tera",
        Some("json_serializable"),
    )
    .expect("Failed to load template");

    let mut context = Context::new();
    context.insert("classes", &classes);
    context.insert("filename", filename);

    let rendered = tera.render("json_serializable", &context).unwrap();
    rendered
}

fn generate_from_json_expression(dart_type: &DartType, access: &str) -> String {
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
            let inner_expr = generate_from_json_expression(inner, element);
            format!(
                "({} as List<dynamic>).map(({}) => {}).toList()",
                access, element, inner_expr
            )
        }
        TypeKind::Map(k, v) => {
            let key = "k";
            let value = "v";
            let key_expr = generate_from_json_expression(k, key);
            let value_expr = generate_from_json_expression(v, value);
            format!(
                "({} as Map<String, dynamic>).map(({}, {}) => MapEntry({}, {}))",
                access, key, value, key_expr, value_expr
            )
        }
        TypeKind::Custom(name) => format!("{}.fromJson({} as Map<String, dynamic>)", name, access),
    };

    if dart_type.is_nullable && !matches!(dart_type.kind, TypeKind::String) {
        format!("{} == null ? null : {}", access, expression)
    } else {
        expression
    }
}

fn generate_to_json_expression(dart_type: &DartType, access: &str) -> String {
    match &dart_type.kind {
        TypeKind::DateTime => {
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}toIso8601String()", access, op)
        }
        TypeKind::Custom(_) => {
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}toJson()", access, op)
        }
        TypeKind::List(inner) => {
            let inner_expr = generate_to_json_expression(inner, "elem");
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!("{}{}map((elem) => {}).toList()", access, op, inner_expr)
        }
        TypeKind::Map(k, v) => {
            let key_expr = generate_to_json_expression(k, "key");
            let value_expr = generate_to_json_expression(v, "value");
            let op = if dart_type.is_nullable { "?." } else { "." };
            format!(
                "{}{}map((key, value) => MapEntry({}, {}))",
                access, op, key_expr, value_expr
            )
        }
        _ => access.to_string(),
    }
}
