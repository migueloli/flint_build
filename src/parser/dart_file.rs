use crate::parser::dart_types::{DartClass, DartField, DartType};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub fn parse_file(path: &Path) -> Result<Vec<DartClass>> {
    let content = fs::read_to_string(path)?;

    // 1. Initialize the parser
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .context("Error loading Dart grammar")?;

    // 2. Parse the code into a Tree
    let tree = parser
        .parse(&content, None)
        .context("Could not parse file")?;

    let mut classes = Vec::new();

    // 3. Define the query (look for classes with ANY annotation)
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
                    let text = &content[inner.start_byte()..inner.end_byte()];

                    if let Some(field) = parse_field(text) {
                        fields.push(field);
                    }
                }
            }
        } else if child.kind() == "field_declaration" || child.kind() == "declaration" {
            let text = &content[child.start_byte()..child.end_byte()];

            if let Some(field) = parse_field(text) {
                fields.push(field);
            }
        }
    }
    fields
}

fn parse_field(text: &str) -> Option<DartField> {
    let parts: Vec<&str> = text.split_whitespace().collect();

    match parts.len() {
        2 => {
            let is_nullable = parts[0].ends_with("?");
            Some(DartField {
                name: parts[1].replace(";", "").to_string(),
                dart_type: parse_dart_type(parts[0]),
                is_nullable: is_nullable,
                is_final: false,
            })
        }
        3 => {
            let is_nullable = parts[1].ends_with("?");
            let is_final = parts[0] == "final";
            Some(DartField {
                name: parts[2].replace(";", "").to_string(),
                dart_type: parse_dart_type(parts[1]),
                is_nullable: is_nullable,
                is_final: is_final,
            })
        }
        _ => None,
    }
}

fn parse_dart_type(type_str: &str) -> DartType {
    let type_str = type_str.trim_end_matches('?');
    match type_str {
        "String" => DartType::String,
        "int" => DartType::Int,
        "double" => DartType::Double,
        "bool" => DartType::Bool,
        "DateTime" => DartType::DateTime,
        _ => DartType::Custom(type_str.to_string()),
    }
}
