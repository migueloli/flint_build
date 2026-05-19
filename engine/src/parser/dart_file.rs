use crate::parser::dart_types::{
    DartClass, DartEnum, DartEnumValue, DartField, DartType, ParsedFile, TypeKind,
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

    Ok(ParsedFile { classes, enums })
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
    let check_metadata = |node: Node<'_>| -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "annotation" {
                let annotation_text = child
                    .child(1)
                    .map(|n| n.utf8_text(content.as_bytes()).unwrap_or(""))
                    .unwrap_or("");

                metadata.insert(annotation_text.to_string(), String::new());

                let mut args_cursor = child.walk();
                for arg_node in child.children(&mut args_cursor) {
                    if arg_node.kind() == "annotation_arguments" {
                        extract_annotation_metadata(&arg_node, content, &mut metadata);
                    }
                }
            }
        }

        metadata
    };

    let mut metadata = check_metadata(field);
    if metadata.is_empty()
        && let Some(parent) = field.parent()
    {
        metadata = check_metadata(parent);
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
            let mut custom_value = None;

            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    variant_name = inner
                        .utf8_text(content.as_bytes())
                        .unwrap_or("")
                        .to_string();
                }

                if inner.kind() == "annotation" {
                    if let Some(val) = process_json_value_node(inner, content) {
                        custom_value = Some(val);
                    }
                } else if inner.kind() == "metadata" {
                    let mut meta_cursor = inner.walk();
                    for meta_child in inner.children(&mut meta_cursor) {
                        if meta_child.kind() == "annotation"
                            && let Some(val) = process_json_value_node(meta_child, content)
                        {
                            custom_value = Some(val);
                        }
                    }
                }
            }

            if !variant_name.is_empty() {
                values.push(DartEnumValue {
                    name: variant_name,
                    value: custom_value,
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
          (annotation
            (_) @annotation_name
            (annotation_arguments)? @annotation_args
          )?
          (_) @class_name
          (type_parameters)? @type_params
          (class_body) @class_body
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
                "annotation_name" => {
                    let text_no_at = text.trim_start_matches('@');
                    metadata.insert(text_no_at.to_string(), String::new());
                }
                "annotation_args" => extract_annotation_metadata(&node, content, &mut metadata),
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
          (annotation
            (_) @enum_annotation_name
            (annotation_arguments)? @enum_annotation_args
          )?
          (_) @enum_name
          (enum_body) @enum_body
        ) @enum_decl
    "#;

    let query = Query::new(&tree_sitter_dart::LANGUAGE.into(), query_str)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, content.as_bytes());

    let mut enums = Vec::new();
    let mut processed_nodes = std::collections::HashSet::new();

    while let Some(m) = matches.next() {
        let enum_decl_node = m.captures.iter()
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
                "enum_annotation_name" => annotations.push(text.to_string()),
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
            }
        "#;
        let tree = parse_snippet(code);
        let classes = extract_classes(tree.root_node(), code).unwrap();

        assert_eq!(classes.len(), 1);
        let class = &classes[0];
        assert_eq!(class.name, "ApiResponse");
        assert_eq!(class.type_parameters, vec!["T", "U"]);
        assert_eq!(class.fields.len(), 2);

        assert_eq!(class.fields[0].name, "data");
        assert_eq!(
            class.fields[0].dart_type.kind,
            TypeKind::Custom("T".to_string())
        );

        assert_eq!(class.fields[1].name, "date");
        assert_eq!(class.fields[1].dart_type.kind, TypeKind::DateTime);
        assert!(class.fields[1].metadata.contains_key("MyConverter"));
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
        assert_eq!(status.values[0].value, None);

        assert_eq!(status.values[1].name, "active");
        assert_eq!(status.values[1].value, Some("active_status".to_string()));

        assert_eq!(status.values[2].name, "suspended");
        assert_eq!(status.values[2].value, None);
    }
}
