use crate::parser::dart_types::{
    DartClass, DartEnum, DartEnumValue, DartEnumValueAnnotation, DartField, DartType, Declaration,
    DeclarationKind, Directive, DirectiveKind, ParsedFile, TypeKind,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use tree_sitter::{Node, Parser, Query, QueryCursor, QueryError, StreamingIterator};

// Compiling a query costs ~3 ms, about 35 times more than parsing a typical model file, so each query is
// compiled once per process and shared by every file and thread (A1). A compile error is kept rather than
// unwrapped, so a bad query is reported as an error instead of a panic.
static CLASS_QUERY: LazyLock<Result<Query, QueryError>> = LazyLock::new(|| {
    Query::new(
        &tree_sitter_dart::LANGUAGE.into(),
        r#"
        (class_declaration
          name: (_) @class_name
          (type_parameters)? @type_params
          body: (class_body) @class_body
        ) @class_decl
        "#,
    )
});

static ENUM_QUERY: LazyLock<Result<Query, QueryError>> = LazyLock::new(|| {
    Query::new(
        &tree_sitter_dart::LANGUAGE.into(),
        r#"
        (enum_declaration
          name: (_) @enum_name
          body: (enum_body) @enum_body
        ) @enum_decl
        "#,
    )
});

fn compiled(query: &'static LazyLock<Result<Query, QueryError>>) -> Result<&'static Query> {
    query
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Invalid built-in tree-sitter query: {e}"))
}

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
        part_of: extract_part_of(tree.root_node(), &content),
        directives: extract_directives(tree.root_node(), &content),
        declarations: extract_declarations(tree.root_node(), &content),
    })
}

fn text<'a>(node: Node, content: &'a str) -> &'a str {
    node.utf8_text(content.as_bytes()).unwrap_or("")
}

/// The target of `part of 'lib.dart';` (or of the legacy `part of my.library;`).
fn extract_part_of(root: Node, content: &str) -> Option<String> {
    let mut cursor = root.walk();
    let directive = root
        .children(&mut cursor)
        .find(|node| node.kind() == "part_of_directive")?;
    let mut inner = directive.walk();
    let target = directive.named_children(&mut inner).next()?;
    Some(unquote(text(target, content)).to_string())
}

fn extract_directives(root: Node, content: &str) -> Vec<Directive> {
    let mut directives = Vec::new();
    let mut cursor = root.walk();
    for node in root
        .children(&mut cursor)
        .filter(|node| node.kind() == "import_or_export")
    {
        let mut inner = node.walk();
        for child in node.named_children(&mut inner) {
            let (kind, spec) = match child.kind() {
                "library_import" => {
                    let mut c = child.walk();
                    let spec = child
                        .named_children(&mut c)
                        .find(|n| n.kind() == "import_specification");
                    (DirectiveKind::Import, spec)
                }
                "library_export" => (DirectiveKind::Export, Some(child)),
                _ => continue,
            };
            let Some(spec) = spec else { continue };
            // `uri:` is a configurable_uri; its first `uri` child is the default target.
            let Some(uri) = spec.child_by_field_name("uri").and_then(|configurable| {
                let mut c = configurable.walk();
                configurable
                    .named_children(&mut c)
                    .find(|n| n.kind() == "uri")
            }) else {
                continue;
            };
            let mut directive = Directive {
                kind,
                uri: unquote(text(uri, content)).to_string(),
                prefix: spec
                    .child_by_field_name("alias")
                    .map(|alias| text(alias, content).to_string()),
                show: Vec::new(),
                hide: Vec::new(),
            };
            let mut c = spec.walk();
            for combinator in spec
                .named_children(&mut c)
                .filter(|n| n.kind() == "combinator")
            {
                let keyword = combinator.child(0).map(|k| text(k, content)).unwrap_or("");
                let mut names_cursor = combinator.walk();
                let names = combinator
                    .named_children(&mut names_cursor)
                    .map(|name| text(name, content).to_string());
                match keyword {
                    "show" => directive.show.extend(names),
                    "hide" => directive.hide.extend(names),
                    _ => {}
                }
            }
            directives.push(directive);
        }
    }
    directives
}

/// Every top-level type declaration, annotated or not, for the project index (spec 0005).
fn extract_declarations(root: Node, content: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        let (kind, name) = match node.kind() {
            // `class M = Object with Mx;` keeps its name inside `mixin_application_class`.
            "class_declaration" => (
                DeclarationKind::Class,
                node.child_by_field_name("name").or_else(|| {
                    let mut c = node.walk();
                    let application = node
                        .named_children(&mut c)
                        .find(|n| n.kind() == "mixin_application_class")?;
                    let mut inner = application.walk();
                    application
                        .named_children(&mut inner)
                        .find(|n| n.kind() == "identifier")
                }),
            ),
            "enum_declaration" => (DeclarationKind::Enum, node.child_by_field_name("name")),
            "mixin_declaration" => (DeclarationKind::Mixin, node.child_by_field_name("name")),
            "extension_type_declaration" => (
                DeclarationKind::ExtensionType,
                node.child_by_field_name("name").and_then(|name| {
                    let mut c = name.walk();
                    name.named_children(&mut c)
                        .find(|n| n.kind() == "identifier")
                }),
            ),
            "type_alias" => {
                let mut c = node.walk();
                let name = node
                    .named_children(&mut c)
                    .find(|n| n.kind() == "type_identifier");
                (DeclarationKind::TypeAlias, name)
            }
            _ => continue,
        };
        let Some(name) = name else { continue };
        let (has_from_json, has_to_json) = match (kind, node.child_by_field_name("body")) {
            (DeclarationKind::Class, Some(body)) => json_members(body, content),
            _ => (false, false),
        };
        declarations.push(Declaration {
            name: text(name, content).to_string(),
            kind,
            has_from_json,
            has_to_json,
        });
    }
    declarations
}

/// Whether a class body declares a `fromJson` constructor or factory and a `toJson` method.
fn json_members(body: Node, content: &str) -> (bool, bool) {
    fn visit(node: Node, content: &str, depth: usize, found: &mut (bool, bool)) {
        if depth > 3 || node.kind() == "function_body" {
            return;
        }
        match node.kind() {
            "constructor_signature"
            | "constant_constructor_signature"
            | "factory_constructor_signature"
            | "redirecting_factory_constructor_signature" => {
                let mut c = node.walk();
                let last_name = node.children_by_field_name("name", &mut c).last();
                if last_name.is_some_and(|name| text(name, content) == "fromJson") {
                    found.0 = true;
                }
            }
            "function_signature" => {
                let name = node
                    .child_by_field_name("name")
                    .map(|name| text(name, content));
                // `static T fromJson(...)` is called like a factory, so it counts; an instance method doesn't.
                let is_static = node.parent().is_some_and(|signature| {
                    let mut c = signature.walk();
                    signature
                        .children(&mut c)
                        .any(|child| child.kind() == "static")
                });
                match name {
                    Some("toJson") => found.1 = true,
                    Some("fromJson") if is_static => found.0 = true,
                    _ => {}
                }
            }
            _ => {
                let mut c = node.walk();
                for child in node.named_children(&mut c) {
                    visit(child, content, depth + 1, found);
                }
            }
        }
    }
    let mut found = (false, false);
    visit(body, content, 0, &mut found);
    found
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

    // The type is the source text from its first to its last type node, so prefixes (`m.Money`) keep
    // their dot and records or function types aren't lost.
    let mut type_span: Option<(usize, usize)> = None;
    let mut name_str = String::new();
    let mut is_final = false;
    let mut is_nullable = false;

    let mut decl_cursor = field.walk();
    for decl_child in field.children(&mut decl_cursor) {
        let kind = decl_child.kind();
        match kind {
            "final" => is_final = true,
            "type_identifier" | "type_arguments" | "record_type" | "function_type"
            | "void_type" => {
                let start = type_span.map_or(decl_child.start_byte(), |(start, _)| start);
                type_span = Some((start, decl_child.end_byte()));
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

    let type_text = type_span.map_or("", |(start, end)| &content[start..end]);
    log::trace!("Resolved type for field {}: {}", name_str, type_text);

    if !name_str.is_empty() {
        return Some(DartField {
            name: name_str,
            line: field.start_position().row + 1,
            dart_type: parse_dart_type(type_text, is_nullable),
            is_final,
            from_json_expr: None,
            to_json_expr: None,
            metadata,
            converter: None,
        });
    }

    None
}

/// Splits `K, V` at the first comma that isn't nested in `<>`, `()` or `{}`.
fn split_type_arguments(arguments: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, c) in arguments.char_indices() {
        match c {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return Some((&arguments[..i], &arguments[i + 1..])),
            _ => {}
        }
    }
    None
}

fn parse_dart_type(type_str: &str, is_nullable: bool) -> DartType {
    let type_str = type_str.trim();

    // Records and fields without a declared type (function types are checked after List/Map are unwrapped).
    if type_str.is_empty() || type_str.starts_with('(') {
        return DartType {
            kind: TypeKind::Unsupported(type_str.to_string()),
            is_nullable,
        };
    }

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
        if let Some((k, v)) = split_type_arguments(inner_content) {
            let (k, v) = (k.trim(), v.trim());
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

    // Function types, plain (`Function(`) or generic (`Function<`).
    if type_str.contains("Function(") || type_str.contains("Function<") {
        return DartType {
            kind: TypeKind::Unsupported(type_str.to_string()),
            is_nullable,
        };
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
                    let literal = first_argument(node, content);
                    annotations.push(DartEnumValueAnnotation {
                        name: name.to_string(),
                        value: literal.as_deref().map(|l| unquote(l).to_string()),
                        literal,
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
                    literal: None,
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

/// The first argument of an annotation, as written in the source: `@JsonValue(1)` → `1`.
fn first_argument(annotation: Node, content: &str) -> Option<String> {
    let mut cursor = annotation.walk();
    let arguments = annotation
        .children(&mut cursor)
        .find(|child| child.kind() == "annotation_arguments")?;
    let mut args_cursor = arguments.walk();
    let argument = arguments
        .children(&mut args_cursor)
        .find(|child| child.kind() == "argument")?;
    let text = argument.utf8_text(content.as_bytes()).ok()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Removes one pair of quotes from a Dart string literal (`'a'`, `"a"`, `'''a'''`, `r'a'`); anything
/// else (numbers, booleans, identifiers) is returned unchanged. Escapes are left as written.
fn unquote(literal: &str) -> &str {
    let raw = literal.strip_prefix('r').unwrap_or(literal);
    for quote in ["'''", "\"\"\"", "'", "\""] {
        if raw.len() >= 2 * quote.len()
            && let Some(inner) = raw
                .strip_prefix(quote)
                .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    literal
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
    let query = compiled(&CLASS_QUERY)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root, content.as_bytes());
    let mut classes = Vec::new();
    let mut processed_nodes = std::collections::HashSet::new();
    while let Some(m) = matches.next() {
        let class_decl_node = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "class_decl")
            .map(|c| c.node);
        if let Some(node) = class_decl_node
            && !processed_nodes.insert(node.id())
        {
            continue;
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
    let query = compiled(&ENUM_QUERY)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root, content.as_bytes());

    let mut enums = Vec::new();
    let mut processed_nodes = std::collections::HashSet::new();

    while let Some(m) = matches.next() {
        let enum_decl_node = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "enum_decl")
            .map(|c| c.node);

        if let Some(node) = enum_decl_node
            && !processed_nodes.insert(node.id())
        {
            continue; // Skip duplicate matches
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
    fn test_enum_value_literals_keep_their_type_and_quotes() {
        // R6: the literal's kind (and any quotes inside it) used to be lost.
        let code = r#"
            enum E {
                @JsonValue(1) a,
                @JsonValue(true) b,
                @JsonValue("it's") c,
                @JsonValue('say "hi"') d,
                @JsonValue(kName) e,
            }
        "#;
        let tree = parse_snippet(code);
        let enums = extract_enums(tree.root_node(), code).unwrap();
        let annotations: Vec<(Option<&str>, Option<&str>)> = enums[0]
            .values
            .iter()
            .map(|v| {
                (
                    v.annotations[0].literal.as_deref(),
                    v.annotations[0].value.as_deref(),
                )
            })
            .collect();

        assert_eq!(
            annotations,
            vec![
                (Some("1"), Some("1")),
                (Some("true"), Some("true")),
                (Some("\"it's\""), Some("it's")),
                (Some("'say \"hi\"'"), Some("say \"hi\"")),
                (Some("kName"), Some("kName")),
            ]
        );
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("'a'"), "a");
        assert_eq!(unquote("\"a\""), "a");
        assert_eq!(unquote("'''a'''"), "a");
        assert_eq!(unquote("r'a\\b'"), "a\\b");
        assert_eq!(unquote("'say \"hi\"'"), "say \"hi\"");
        assert_eq!(unquote("12"), "12");
        assert_eq!(unquote("red"), "red");
        assert_eq!(unquote("'"), "'");
    }

    #[test]
    fn test_field_types_keep_prefixes_and_flag_unsupported() {
        let code = "class C {\n  final m.Money price;\n  final List<m.Money>? prices;\n  final Map<String, m.Money> byName;\n  final (int, String) pair;\n  final void Function(int) callback;\n  final Map<String, (int, int)> nested;\n  final Map<Map<String, int>, int> keyed;\n  final void Function<T>(T) generic;\n  final List<void Function<T>(T)> generics;\n}\n";
        let tree = parse_snippet(code);
        let classes = extract_classes(tree.root_node(), code).unwrap();
        let fields: Vec<(&str, String, usize)> = classes[0]
            .fields
            .iter()
            .map(|f| (f.name.as_str(), format!("{:?}", f.dart_type.kind), f.line))
            .collect();

        let custom = |name: &str| format!("{:?}", TypeKind::Custom(name.to_string()));
        assert_eq!(fields[0], ("price", custom("m.Money"), 2));
        assert!(fields[1].1.starts_with("List(") && fields[1].1.contains("m.Money"));
        assert!(classes[0].fields[1].dart_type.is_nullable);
        assert!(fields[2].1.starts_with("Map(") && fields[2].1.contains("m.Money"));
        assert_eq!(
            fields[3],
            (
                "pair",
                format!("{:?}", TypeKind::Unsupported("(int, String)".into())),
                5
            )
        );
        assert_eq!(
            fields[4].1,
            format!("{:?}", TypeKind::Unsupported("void Function(int)".into()))
        );
        // The comma inside the record doesn't split the map's type arguments.
        assert_eq!(
            classes[0].fields[5].dart_type.to_string(),
            "Map<String, (int, int)>"
        );
        assert_eq!(
            classes[0].fields[6].dart_type.to_string(),
            "Map<Map<String, int>, int>"
        );
        // Generic function types read `Function<`, not `Function(`.
        assert_eq!(
            fields[7].1,
            format!("{:?}", TypeKind::Unsupported("void Function<T>(T)".into()))
        );
        let TypeKind::List(inner) = &classes[0].fields[8].dart_type.kind else {
            panic!("expected a List");
        };
        assert!(matches!(inner.kind, TypeKind::Unsupported(_)));
    }

    #[test]
    fn test_extract_directives_and_declarations() {
        let code = r#"
import 'dart:convert';
import 'src/money.dart' as m;
import 'package:app/a.dart' show A, B hide C;
export 'src/color.dart';
part 'x.g.dart';
typedef Json = Map<String, dynamic>;
mixin Mx {}
extension type Id(int value) {}
enum Color { red }
class Model {
  Model.fromJson(Map<String, dynamic> json);
  Map<String, dynamic> toJson() => {};
}
class Plain {
  factory Plain.other() => Plain();
  Plain();
}
class Factory {
  factory Factory.fromJson(Map<String, dynamic> json) => Factory();
  Factory();
}
sealed class Shape {
  static Shape fromJson(Map<String, dynamic> json) => throw json;
  Shape fromJsonCopy() => this;
}
class Instance {
  Instance fromJson() => this;
}
class Applied = Object with Mx;
"#;
        let tree = parse_snippet(code);
        let directives = extract_directives(tree.root_node(), code);
        let summary: Vec<(DirectiveKind, &str, Option<&str>)> = directives
            .iter()
            .map(|d| (d.kind, d.uri.as_str(), d.prefix.as_deref()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (DirectiveKind::Import, "dart:convert", None),
                (DirectiveKind::Import, "src/money.dart", Some("m")),
                (DirectiveKind::Import, "package:app/a.dart", None),
                (DirectiveKind::Export, "src/color.dart", None),
            ]
        );
        assert_eq!(directives[2].show, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(directives[2].hide, vec!["C".to_string()]);

        let found = extract_declarations(tree.root_node(), code);
        let declarations: Vec<(&str, DeclarationKind, bool, bool)> = found
            .iter()
            .map(|d| (d.name.as_str(), d.kind, d.has_from_json, d.has_to_json))
            .collect();
        assert_eq!(
            declarations,
            vec![
                ("Json", DeclarationKind::TypeAlias, false, false),
                ("Mx", DeclarationKind::Mixin, false, false),
                ("Id", DeclarationKind::ExtensionType, false, false),
                ("Color", DeclarationKind::Enum, false, false),
                ("Model", DeclarationKind::Class, true, true),
                ("Plain", DeclarationKind::Class, false, false),
                ("Factory", DeclarationKind::Class, true, false),
                // A static `fromJson` also works as `Shape.fromJson(...)`; an instance method doesn't.
                ("Shape", DeclarationKind::Class, true, false),
                ("Instance", DeclarationKind::Class, false, false),
                ("Applied", DeclarationKind::Class, false, false),
            ]
        );

        let part = "part of 'model.dart';\n";
        let tree = parse_snippet(part);
        assert_eq!(
            extract_part_of(tree.root_node(), part),
            Some("model.dart".to_string())
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.dart");
        std::fs::write(&path, "class ApiResponse { final invalid }").unwrap();

        let error = parse_file(&path).unwrap_err().to_string();

        assert!(error.contains("Syntax Error"), "{error}");
        assert!(error.contains("line 1"), "{error}");
    }
}
