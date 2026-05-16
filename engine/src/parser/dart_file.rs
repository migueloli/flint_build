use crate::config::PluginConfig;
use crate::parser::dart_types::{
    DartClass, DartEnum, DartEnumValue, DartField, DartType, ParsedFile, TypeKind,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub fn parse_file(path: &Path, plugin: &PluginConfig) -> Result<ParsedFile> {
    log::debug!("Tree-Sitter: Parsing file {:?}", path);
    let content = fs::read_to_string(path)?;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .context("Error loading Dart grammar")?;

    let tree = parser
        .parse(&content, None)
        .context("Could not parse file")?;

    if tree.root_node().has_error() {
        if let Some(error_node) = find_error_node(tree.root_node()) {
            let start = error_node.start_position();

            // Extract the exact line of code where the error happened
            let lines: Vec<&str> = content.lines().collect();
            let error_line = lines.get(start.row).unwrap_or(&"");

            // Create a pointer string (like "      ^")
            let pointer = " ".repeat(start.column) + "^";

            let msg = format!(
                "Syntax Error in {:?} at line {}, column {}\n\n{}\n{}\n",
                path,
                start.row + 1,
                start.column + 1,
                error_line,
                pointer
            );

            return Err(anyhow::anyhow!(msg));
        }
    }

    let mut classes = Vec::new();
    let mut enums = Vec::new();

    let query_str = r#"
        (class_declaration
          (annotation
            (_) @annotation_name
            (annotation_arguments)? @annotation_args
          )?
          (_) @class_name
          (type_parameters)? @type_params
          (class_body) @class_body
        )
        (enum_declaration
          (annotation
            (_) @enum_annotation_name
            (annotation_arguments)? @enum_annotation_args
          )?
          (_) @enum_name
          (enum_body) @enum_body
        ) @enum_decl
    "#;

    let query = Query::new(&tree_sitter_dart::LANGUAGE.into(), query_str)
        .context("Failed to create query")?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    while let Some(m) = matches.next() {
        let mut class_name = String::new();
        let mut has_matching_annotation = false;
        let mut class_body_node = None;
        let mut metadata = HashMap::new();
        let mut type_parameters = Vec::new();

        let mut enum_name = String::new();
        let mut enum_body_node = None;
        let mut is_enum = false;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];
            let text = &content[node.start_byte()..node.end_byte()];
            let full_annotation = format!("@{}", text);
            if capture_name == "annotation_name"
                && plugin.class_annotations.contains(&full_annotation)
            {
                has_matching_annotation = true;
            }
            if capture_name == "annotation_args" {
                extract_annotation_metadata(&node, &content, &mut metadata);
            }
            if capture_name == "class_name" {
                class_name = text.to_string();
            }
            if capture_name == "class_body" {
                class_body_node = Some(node);
            }
            if capture_name == "type_params" {
                let mut tp_cursor = node.walk();
                for child in node.children(&mut tp_cursor) {
                    if child.kind() == "type_parameter" {
                        let mut inner_cursor = child.walk();
                        for inner in child.children(&mut inner_cursor) {
                            if inner.kind() == "type_identifier" {
                                if let Ok(tp_name) = inner.utf8_text(content.as_bytes()) {
                                    type_parameters.push(tp_name.to_string());
                                }
                            }
                        }
                    }
                }
            }

            if capture_name == "enum_annotation_name"
                && plugin.enum_annotations.contains(&full_annotation)
            {
                is_enum = true;
            }
            if capture_name == "enum_name" {
                enum_name = text.to_string();
            }
            if capture_name == "enum_body" {
                enum_body_node = Some(node);
            }
        }
        if has_matching_annotation {
            let mut fields = Vec::new();
            if let Some(body) = class_body_node {
                fields = extract_fields_from_tree(body, &content, plugin);
            }
            classes.push(DartClass {
                name: class_name,
                fields,
                metadata,
                type_parameters,
            });
        }
        log::debug!("Found enum: {}", enum_name);
        if is_enum {
            if let Some(body) = enum_body_node {
                let dart_enum = extract_enum_values(enum_name, body, &content, plugin);
                enums.push(dart_enum);
            }
        }
    }

    if classes.is_empty() {
        log::debug!("No classes with matching annotations found in {:?}", path);
    } else {
        log::debug!("Found {} classes in {:?}", classes.len(), path);
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

fn extract_fields_from_tree(body: Node, content: &str, plugin: &PluginConfig) -> Vec<DartField> {
    let mut fields = Vec::new();
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "class_member" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "declaration" {
                    if let Some(field) = parse_field(inner, content, plugin) {
                        fields.push(field);
                    }
                }
            }
        } else if child.kind() == "field_declaration" || child.kind() == "declaration" {
            if let Some(field) = parse_field(child, content, plugin) {
                fields.push(field);
            }
        }
    }
    fields
}

fn parse_field(field: Node<'_>, content: &str, plugin: &PluginConfig) -> Option<DartField> {
    let check_metadata = |node: Node<'_>| -> (HashMap<String, String>, Option<String>) {
        let mut metadata = HashMap::new();
        let mut converter = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "annotation" {
                let annotation_text = child
                    .child(1)
                    .map(|n| n.utf8_text(content.as_bytes()).unwrap_or(""))
                    .unwrap_or("");
                let full_annotation = format!("@{}", annotation_text);

                if plugin.field_annotations.contains(&full_annotation) {
                    let mut args_cursor = child.walk();
                    for arg_node in child.children(&mut args_cursor) {
                        if arg_node.kind() == "annotation_arguments" {
                            extract_annotation_metadata(&arg_node, content, &mut metadata);
                        }
                    }
                } else if let Some(converters) = &plugin.converters {
                    if converters.contains(&full_annotation) {
                        converter = Some(annotation_text.to_string());
                    }
                }
            }
        }

        (metadata, converter)
    };

    // First check the field node itself
    let (mut metadata, mut converter) = check_metadata(field);

    // If not found, check the parent node (usually `class_member`)
    if metadata.is_empty() && converter.is_none() {
        if let Some(parent) = field.parent() {
            let (m, c) = check_metadata(parent);
            metadata = m;
            converter = c;
        }
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

    log::trace!("Resolved type for field {}: {}", name_str, type_parts);

    if !name_str.is_empty() {
        return Some(DartField {
            name: name_str,
            dart_type: parse_dart_type(&type_parts, is_nullable),
            is_final,
            from_json_expr: None,
            to_json_expr: None,
            metadata: metadata,
            converter: converter,
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

fn extract_enum_values(name: String, body: Node, content: &str, plugin: &PluginConfig) -> DartEnum {
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
                    if let Some(val) = process_json_value_node(inner, content, plugin) {
                        custom_value = Some(val);
                    }
                } else if inner.kind() == "metadata" {
                    let mut meta_cursor = inner.walk();
                    for meta_child in inner.children(&mut meta_cursor) {
                        if meta_child.kind() == "annotation" {
                            if let Some(val) = process_json_value_node(meta_child, content, plugin)
                            {
                                custom_value = Some(val);
                            }
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
    DartEnum { name, values }
}

fn process_json_value_node(node: Node, content: &str, plugin: &PluginConfig) -> Option<String> {
    let mut cursor = node.walk();
    let mut name = String::new();
    let mut args = String::new();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "type_identifier" => {
                name = child
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();
            }
            "arguments" | "annotation_arguments" => {
                args = child
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();
            }
            _ => {}
        }
    }
    let full_annotation = format!("@{}", name);
    if plugin.variant_annotations.contains(&full_annotation) {
        let val = args.trim_matches(|c| c == '(' || c == ')' || c == '"' || c == '\'' || c == ' ');
        return Some(val.to_string());
    }
    None
}

fn find_error_node<'a>(node: Node<'a>) -> Option<Node<'a>> {
    // If this specific node is an error or is missing expected syntax, we found it!
    if node.is_error() || node.is_missing() {
        return Some(node);
    }

    // Otherwise, recursively check all children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(err) = find_error_node(child) {
            return Some(err);
        }
    }
    None
}
