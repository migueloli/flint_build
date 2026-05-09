use crate::parser::dart_types::{DartClass, DartType, TypeKind};

pub fn generate_full_file(filename: &str, classes: &[DartClass]) -> String {
    let mut code = String::new();

    code.push_str("// GENERATED CODE - DO NOT MODIFY BY HAND\n\n");
    code.push_str(&format!("part of '{}';\n\n", filename));

    code.push_str(&format!("// {}\n", "*".repeat(74)));
    code.push_str("// JsonSerializableGenerator (Powered by Flint)\n");
    code.push_str(&format!("// {}\n\n", "*".repeat(74)));

    for (i, class) in classes.iter().enumerate() {
        code.push_str(&generate_json_code(class));
        if i < classes.len() - 1 {
            code.push_str("\n\n");
        }
    }
    code.push_str("\n");

    code
}

pub fn generate_json_code(class: &DartClass) -> String {
    let mut code = String::new();

    let name = &class.name;

    // FromJson
    code.push_str(&format!(
        "{} _${}FromJson(Map<String, dynamic> json) => {}(\n",
        name, name, name
    ));
    for field in &class.fields {
        let key = field.json_key.as_ref().unwrap_or(&field.name);
        let json_access = format!("json['{}']", key);
        let expression = generate_from_json_expression(&field.dart_type, &json_access);
        code.push_str(&format!("  {}: {},\n", field.name, expression));
    }
    code.push_str(");\n\n");

    // ToJson
    code.push_str(&format!(
        "Map<String, dynamic> _${}ToJson({} instance) => <String, dynamic>{{\n",
        name, name
    ));
    for field in &class.fields {
        let key = field.json_key.as_ref().unwrap_or(&field.name);
        let json_access = format!("instance.{}", field.name);
        let expression = generate_to_json_expression(&field.dart_type, &json_access);
        code.push_str(&format!("  '{}': {},\n", key, expression));
    }
    code.push_str("};");

    code
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
