use crate::parser::dart_types::{
    DartClass, DartEnum, DartEnumValue, DartEnumValueAnnotation, DartField, DartType, ParsedFile,
    TypeKind,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub fn parse_file(path: &Path) -> Result<ParsedFile> {
    log::debug!("Tree-Sitter: Parsing file {:?}", path);
    let content = fs::read_to_string(path)?;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .context("Error loading Dart grammar")?;

    let tree = parser
        .parse(&content, None)
        .context("Could not parse file")?;

    if tree.root_node().has_error()
        && let Some(error_node) = find_error_node(tree.root_node())
    {
        let start = error_node.start_position();
        let lines: Vec<&str> = content.lines().collect();
        let error_line = lines.get(start.row).unwrap_or(&"");
        let pointer = " ".repeat(start.column) + "^";
        return Err(crate::error::FlintError::Syntax {
            file: path.display().to_string(),
            line: start.row + 1,
            column: start.column + 1,
            source_line: error_line.to_string(),
            pointer,
        }
        .into());
    }

    let classes = extract_classes(tree.root_node(), &content)?;
    let enums = extract_enums(tree.root_node(), &content)?;

    if classes.is_empty() {
        log::debug!("No classes with matching annotations found in {:?}", path);
    } else {
        log::debug!("Found {} classes in {:?}", classes.len(), path);
    }

    if enums.is_empty() {
        log::debug!("No enums with matching annotations found in {:?}", path);
    } else {
        log::debug!("Found {} enums in {:?}", enums.len(), path);
    }

    Ok(ParsedFile {
        classes,
        enums,
        part_directives: extract_part_directives(tree.root_node(), &content),
    })
}

fn extract_part_directives(root: Node, content: &str) -> Vec<String> {
    let mut cursor = root.walk();
    root.children(&mut cursor)
        .filter(|node| node.kind() == "part_directive")
        .filter_map(|node| node.child_by_field_name("uri"))
        .filter_map(|uri| uri.utf8_text(content.as_bytes()).ok())
        .map(|uri| uri.trim_matches(|c| c == '\'' || c == '"').to_string())
        .collect()
}

fn extract_annotation_metadata(
    arg_node: &Node,
    content: &str,
    metadata: &mut HashMap<String, String>,
) {
    let mut param_cursor = arg_node.walk();
    for param in arg_node.children(&mut param_cursor) {
        if param.kind() == "argument" {
            let mut inner_cursor = param.walk();
            for inner_param in param.children(&mut inner_cursor) {
                if inner_param.kind() == "named_argument" {
                    let key = inner_param
                        .child(0)
                        .map(|n| n.utf8_text(content.as_bytes()).unwrap_or(""))
                        .unwrap_or("");
                    let clean_key = key.trim_end_matches(':');

                    let val = inner_param
                        .child(1)
                        .map(|n| n.utf8_text(content.as_bytes()).unwrap_or(""))
                        .unwrap_or("");

                    metadata.insert(clean_key.to_string(), val.to_string());
                }
            }
        }
    }
}

/// Reads every annotation written directly on `node`: their names (without `@`) in source order, and a
/// metadata map with each name → `""` plus the named arguments of all of them.
fn read_annotations(node: Node, content: &str) -> (Vec<String>, HashMap<String, String>) {
    let mut names = Vec::new();
    let mut metadata = HashMap::new();
    let mut cursor = node.walk();
    for annotation in node
        .children(&mut cursor)
        .filter(|child| child.kind() == "annotation")
    {
        let Some(name) = annotation
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(content.as_bytes()).ok())
        else {
            continue;
        };
        names.push(name.to_string());
        metadata.insert(name.to_string(), String::new());

        let mut args_cursor = annotation.walk();
        for arguments in annotation
            .children(&mut args_cursor)
            .filter(|child| child.kind() == "annotation_arguments")
        {
            extract_annotation_metadata(&arguments, content, &mut metadata);
        }
    }
    (names, metadata)
}

fn extract_fields_from_tree(body: Node, content: &str) -> Vec<DartField> {
    let mut fields = Vec::new();
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "class_member" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "declaration"
                    && let Some(field) = parse_field(inner, content)
                {
                    fields.push(field);
                }
            }
        } else if (child.kind() == "field_declaration" || child.kind() == "declaration")
            && let Some(field) = parse_field(child, content)
        {
            fields.push(field);
        }
    }
    fields
}

fn parse_field(field: Node<'_>, content: &str) -> Option<DartField> {
    // Field annotations are usually siblings of the declaration, inside its `class_member`.
    let (_, mut metadata) = read_annotations(field, content);
    if metadata.is_empty()
        && let Some(parent) = field.parent()
    {
        metadata = read_annotations(parent, content).1;
    }

    let mut type_parts = String::new();
    let mut name_str = String::new();
    let mut is_final = false;
    let mut is_nullable = false;

    let mut decl_cursor = field.walk();
    for decl_child in field.children(&mut decl_cursor) {
        let kind = decl_child.kind();
        let text = &content[decl_child.start_byte()..decl_child.end_byte()];
        match kind {
            "final" => is_final = true,
            "type_identifier" | "type_arguments" => {
                type_parts.push_str(text);
            }
            "?" => is_nullable = true,
            "initialized_identifier_list" => {
                if let Some(init_id) = decl_child.child(0)
                    && let Some(name_node) = init_id.child(0)
                {
                    name_str = content[name_node.start_byte()..name_node.end_byte()].to_string();
                }
            }
            _ => {}
        }
    }

    log::trace!("Resolved type for field {}: {}", name_str, type_parts);

    if !name_str.is_empty() {
        return Some(DartField {
            name: name_str,
            dart_type: parse_dart_type(&type_parts, is_nullable),
            is_final,
            from_json_expr: None,
            to_json_expr: None,
            metadata,
            converter: None,
        });
    }

    None
}

fn parse_dart_type(type_str: &str, is_nullable: bool) -> DartType {
    let type_str = type_str.trim();

    if type_str.starts_with("List<") && type_str.ends_with('>') {
        let inner_type = &type_str[5..type_str.len() - 1];
        let is_inner_nullable = inner_type.ends_with('?');
        return DartType {
            kind: TypeKind::List(Box::new(parse_dart_type(
                inner_type.trim().trim_end_matches('?'),
                is_inner_nullable,
            ))),
            is_nullable,
        };
    }

    if type_str.starts_with("Map<") && type_str.ends_with('>') {
        let inner_content = &type_str[4..type_str.len() - 1];
        if let Some((k, v)) = inner_content.split_once(',') {
            let is_key_nullable = k.ends_with('?');
            let is_value_nullable = v.ends_with('?');
            return DartType {
                kind: TypeKind::Map(
                    Box::new(parse_dart_type(
                        k.trim().trim_end_matches('?'),
                        is_key_nullable,
                    )),
                    Box::new(parse_dart_type(
                        v.trim().trim_end_matches('?'),
                        is_value_nullable,
                    )),
                ),
                is_nullable,
            };
        }
    }

    match type_str {
        "String" => DartType {
            kind: TypeKind::String,
            is_nullable,
        },
        "int" => DartType {
            kind: TypeKind::Int,
            is_nullable,
        },
        "double" => DartType {
            kind: TypeKind::Double,
            is_nullable,
        },
        "bool" => DartType {
            kind: TypeKind::Bool,
            is_nullable,
        },
        "DateTime" => DartType {
            kind: TypeKind::DateTime,
            is_nullable,
        },
        _ => DartType {
            kind: TypeKind::Custom(type_str.to_string()),
            is_nullable,
        },
    }
}

fn extract_enum_values(name: String, body: Node, content: &str) -> DartEnum {
    let mut values = Vec::new();
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "enum_constant" {
            let mut variant_name = String::new();
            let mut annotations = Vec::new();
            let mut push_annotation = |node: Node| {
                if let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|name| name.utf8_text(content.as_bytes()).ok())
                {
                    annotations.push(DartEnumValueAnnotation {
                        name: name.to_string(),
                        value: process_json_value_node(node, content),
                    });
                }
            };

            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    variant_name = inner
                        .utf8_text(content.as_bytes())
                        .unwrap_or("")
                        .to_string();
                }

                if inner.kind() == "annotation" {
                    push_annotation(inner);
                } else if inner.kind() == "metadata" {
                    let mut meta_cursor = inner.walk();
                    for meta_child in inner.children(&mut meta_cursor) {
                        if meta_child.kind() == "annotation" {
                            push_annotation(meta_child);
                        }
                    }
                }
            }

            if !variant_name.is_empty() {
                values.push(DartEnumValue {
                    name: variant_name,
                    value: None,
                    annotations,
                });
            }
        }
    }
    DartEnum {
        name,
        annotations: Vec::new(),
        values,
    }
}

fn process_json_value_node(node: Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    let mut args = String::new();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "arguments" | "annotation_arguments" => {
                args = child
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();
            }
            _ => {}
        }
    }
    let val = args.trim_matches(|c| c == '(' || c == ')' || c == '"' || c == '\'' || c == ' ');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

fn find_error_node<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(err) = find_error_node(child) {
            return Some(err);
        }
    }
    None
}

fn extract_classes(root: Node, content: &str) -> Result<Vec<DartClass>> {
    let query_str = r#"
        (class_declaration
          name: (_) @class_name
          (type_parameters)? @type_params
          body: (class_body) @class_body
        ) @class_decl
    "#;

    let query = Query::new(&tree_sitter_dart::LANGUAGE.into(), query_str)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, content.as_bytes());
    let mut classes = Vec::new();
    let mut processed_nodes = std::collections::HashSet::new();
    while let Some(m) = matches.next() {
        let class_decl_node = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "class_decl")
            .map(|c| c.node);
        if let Some(node) = class_decl_node {
            if !processed_nodes.insert(node.id()) {
                continue;
            }
        }

        let mut class_name = String::new();
        let mut class_body_node = None;
        let mut metadata = HashMap::new();
        let mut type_parameters = Vec::new();

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];
            let text = &content[node.start_byte()..node.end_byte()];

            match capture_name {
                "class_decl" => metadata = read_annotations(node, content).1,
                "class_name" => class_name = text.to_string(),
                "type_params" => type_parameters = extract_type_parameters(node, content),
                "class_body" => class_body_node = Some(node),
                _ => {}
            }
        }

        let fields = class_body_node
            .map(|body| extract_fields_from_tree(body, content))
            .unwrap_or_default();

        classes.push(DartClass {
            name: class_name,
            fields,
            metadata,
            type_parameters,
        });
    }
    Ok(classes)
}

fn extract_type_parameters(node: Node, content: &str) -> Vec<String> {
    let mut type_parameters = Vec::new();
    let mut tp_cursor = node.walk();

    for child in node.children(&mut tp_cursor) {
        if child.kind() == "type_parameter" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "type_identifier"
                    && let Ok(tp_name) = inner.utf8_text(content.as_bytes())
                {
                    type_parameters.push(tp_name.to_string());
                }
            }
        }
    }
    type_parameters
}

fn extract_enums(root: Node, content: &str) -> Result<Vec<DartEnum>> {
    let query_str = r#"
        (enum_declaration
          name: (_) @enum_name
          body: (enum_body) @enum_body
        ) @enum_decl
    "#;

    let query = Query::new(&tree_sitter_dart::LANGUAGE.into(), query_str)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, content.as_bytes());

    let mut enums = Vec::new();
    let mut processed_nodes = std::collections::HashSet::new();

    while let Some(m) = matches.next() {
        let enum_decl_node = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "enum_decl")
            .map(|c| c.node);

        if let Some(node) = enum_decl_node {
            if !processed_nodes.insert(node.id()) {
                continue; // Skip duplicate matches
            }
        }

        let mut enum_name = String::new();
        let mut enum_body_node = None;
        let mut annotations = Vec::new();

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];
            let text = &content[node.start_byte()..node.end_byte()];

            match capture_name {
                "enum_decl" => annotations = read_annotations(node, content).0,
                "enum_name" => enum_name = text.to_string(),
                "enum_body" => enum_body_node = Some(node),
                _ => {}
            }
        }

        if let Some(body) = enum_body_node {
            let mut dart_enum = extract_enum_values(enum_name, body, content);
            dart_enum.annotations = annotations;
            enums.push(dart_enum);
        }
    }
    Ok(enums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dart_types::TypeKind;
    use tree_sitter::Parser;

    fn parse_snippet(code: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        parser.parse(code, None).unwrap()
    }

    #[test]
    fn test_extract_classes() {
        let code = r#"
            @JsonSerializable()
            class ApiResponse<T, U> {
                final T data;
                @MyConverter()
                final DateTime date;
                final Map<String, int>? dataMap;
            }
        "#;
        let tree = parse_snippet(code);
        let classes = extract_classes(tree.root_node(), code).unwrap();

        assert_eq!(classes.len(), 1);
        let class = &classes[0];
        assert_eq!(class.name, "ApiResponse");
        assert_eq!(class.type_parameters, vec!["T", "U"]);
        assert_eq!(class.fields.len(), 3);

        assert_eq!(class.fields[0].name, "data");
        assert_eq!(
            class.fields[0].dart_type.kind,
            TypeKind::Custom("T".to_string())
        );

        assert_eq!(class.fields[1].name, "date");
        assert_eq!(class.fields[1].dart_type.kind, TypeKind::DateTime);
        assert!(class.fields[1].metadata.contains_key("MyConverter"));

        assert_eq!(class.fields[2].name, "dataMap");
        if let TypeKind::Map(key, val) = &class.fields[2].dart_type.kind {
            assert_eq!(key.kind, TypeKind::String);
            assert_eq!(val.kind, TypeKind::Int);
        } else {
            panic!("Expected Map type");
        }
        assert!(class.fields[2].dart_type.is_nullable);
    }

    #[test]
    fn test_extract_enums() {
        let code = r#"
            @JsonEnum()
            enum UserStatus {
                pending,
                @JsonValue("active_status")
                active,
                suspended,
            }
        "#;
        let tree = parse_snippet(code);
        let enums = extract_enums(tree.root_node(), code).unwrap();

        assert_eq!(enums.len(), 1);
        let status = &enums[0];
        assert_eq!(status.name, "UserStatus");
        assert_eq!(status.annotations, vec!["JsonEnum".to_string()]);
        assert_eq!(status.values.len(), 3);

        assert_eq!(status.values[0].name, "pending");
        assert!(status.values[0].annotations.is_empty());

        // The parser records the annotation; generators pick the value (select_variant_values).
        assert_eq!(status.values[1].name, "active");
        assert_eq!(status.values[1].value, None);
        assert_eq!(status.values[1].annotations[0].name, "JsonValue");
        assert_eq!(
            status.values[1].annotations[0].value,
            Some("active_status".to_string())
        );

        assert_eq!(status.values[2].name, "suspended");
        assert!(status.values[2].annotations.is_empty());
    }

    #[test]
    fn test_class_keeps_every_annotation() {
        // R3: only the first annotation used to be read, so `@immutable` hid `@JsonSerializable`.
        let code = r#"
            @immutable
            @JsonSerializable(explicitToJson: true)
            class A {
                @Deprecated('old')
                @JsonKey(name: 'b_')
                final int b;
            }

            @JsonSerializable()
            @immutable
            class B {}
        "#;
        let tree = parse_snippet(code);
        let classes = extract_classes(tree.root_node(), code).unwrap();

        assert_eq!(classes.len(), 2);
        // The positional `(_) @class_name` capture used to match the second annotation instead.
        assert_eq!(classes[0].name, "A");
        assert_eq!(classes[1].name, "B");
        let a = &classes[0];
        assert!(a.metadata.contains_key("immutable"));
        assert!(a.metadata.contains_key("JsonSerializable"));
        assert_eq!(a.metadata["explicitToJson"], "true");
        assert!(a.fields[0].metadata.contains_key("Deprecated"));
        assert_eq!(a.fields[0].metadata["name"], "'b_'");
        assert!(classes[1].metadata.contains_key("JsonSerializable"));
        assert!(classes[1].metadata.contains_key("immutable"));
    }

    #[test]
    fn test_enum_keeps_every_annotation() {
        let code = r#"
            @Tag()
            @JsonEnum()
            enum Status { active }
        "#;
        let tree = parse_snippet(code);
        let enums = extract_enums(tree.root_node(), code).unwrap();

        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Status");
        assert_eq!(
            enums[0].annotations,
            vec!["Tag".to_string(), "JsonEnum".to_string()]
        );
    }

    #[test]
    fn test_extract_part_directives() {
        let code =
            "part 'user.g.dart';\npart \"user.freezed.dart\";\npart of 'lib.dart';\nclass A {}\n";
        let tree = parse_snippet(code);
        assert_eq!(
            extract_part_directives(tree.root_node(), code),
            vec!["user.g.dart".to_string(), "user.freezed.dart".to_string()]
        );
    }

    #[test]
    fn test_parse_file_syntax_error() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("invalid.dart");
        std::fs::write(&path, "class ApiResponse { final invalid }").unwrap();

        let res = parse_file(&path);
        let _ = std::fs::remove_file(&path);

        assert!(res.is_err());
    }
}
