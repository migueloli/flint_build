use crate::parser::dart_types::{DartClass, DartField, DartType};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub fn parse_file(path: &Path) -> Result<Vec<DartClass>> {
    log::debug!("Tree-Sitter: Parsing file {:?}", path);
    let content = fs::read_to_string(path)?;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .context("Error loading Dart grammar")?;

    let tree = parser
        .parse(&content, None)
        .context("Could not parse file")?;

    let mut classes = Vec::new();

    let query_str = r#"
        (class_declaration
          (annotation
            (_) @annotation_name
          )
          (_) @class_name
          (class_body) @class_body
        )
    "#;

    let query = Query::new(&tree_sitter_dart::LANGUAGE.into(), query_str)
        .context("Failed to create query")?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    while let Some(m) = matches.next() {
        let mut class_name = String::new();
        let mut is_json_serializable = false;
        let mut class_body_node = None;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];
            let text = &content[node.start_byte()..node.end_byte()];
            if capture_name == "annotation_name"
                && (text == "JsonSerializable" || text == "jsonSerializable")
            {
                is_json_serializable = true;
            }
            if capture_name == "class_name" {
                class_name = text.to_string();
            }
            if capture_name == "class_body" {
                class_body_node = Some(node);
            }
        }
        if is_json_serializable {
            log::info!(
                "Found @JsonSerializable class: {} in {:?}",
                class_name,
                path
            );
            let mut fields = Vec::new();
            if let Some(body) = class_body_node {
                fields = extract_fields_from_tree(body, &content);
            }
            classes.push(DartClass {
                name: class_name,
                fields,
            });
        }
    }

    Ok(classes)
}

fn extract_fields_from_tree(body: Node, content: &str) -> Vec<DartField> {
    let mut fields = Vec::new();
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "class_member" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "declaration" {
                    if let Some(field) = parse_field(inner, content) {
                        fields.push(field);
                    }
                }
            }
        } else if child.kind() == "field_declaration" || child.kind() == "declaration" {
            if let Some(field) = parse_field(child, content) {
                fields.push(field);
            }
        }
    }
    fields
}

fn parse_field(field: Node<'_>, content: &str) -> Option<DartField> {
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
                if let Some(init_id) = decl_child.child(0) {
                    if let Some(name_node) = init_id.child(0) {
                        name_str =
                            content[name_node.start_byte()..name_node.end_byte()].to_string();
                    }
                }
            }
            _ => {}
        }
    }

    if !name_str.is_empty() {
        return Some(DartField {
            name: name_str,
            dart_type: parse_dart_type(&type_parts),
            is_nullable,
            is_final,
        });
    }

    None
}

fn parse_dart_type(type_str: &str) -> DartType {
    let type_str = type_str.trim();

    if type_str.starts_with("List<") && type_str.ends_with('>') {
        let inner_type = &type_str[5..type_str.len() - 1];
        return DartType::List(Box::new(parse_dart_type(inner_type)));
    }

    if type_str.starts_with("Map<") && type_str.ends_with('>') {
        let inner_content = &type_str[4..type_str.len() - 1];
        if let Some((k, v)) = inner_content.split_once(',') {
            return DartType::Map(
                Box::new(parse_dart_type(k.trim())),
                Box::new(parse_dart_type(v.trim())),
            );
        }
    }

    match type_str {
        "String" => DartType::String,
        "int" => DartType::Int,
        "double" => DartType::Double,
        "bool" => DartType::Bool,
        "DateTime" => DartType::DateTime,
        _ => DartType::Custom(type_str.to_string()),
    }
}
