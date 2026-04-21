use crate::parser::dart_types::DartClass;

pub fn generate_json_code(class: &DartClass) -> String {
    let name = &class.name;
    let mut code = String::new();

    // FromJson
    code.push_str(&format!(
        "{} _${}FromJson(Map<String, dynamic> json) => {}(\n",
        name, name, name
    ));
    for field in &class.fields {
        let null_suffix = if field.is_nullable { "?" } else { "" };
        code.push_str(&format!(
            "      {}: json['{}'] as {}{},\n",
            field.name, field.name, field.dart_type, null_suffix
        ));
    }
    code.push_str("    );\n\n");

    // ToJson
    code.push_str(&format!(
        "Map<String, dynamic> _${}ToJson({} instance) => <String, dynamic>{{\n",
        name, name
    ));
    for field in &class.fields {
        code.push_str(&format!(
            "      '{}': instance.{},\n",
            field.name, field.name
        ));
    }
    code.push_str("    };\n");

    code
}
