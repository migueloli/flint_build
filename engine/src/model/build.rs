//! Builds a [`Library`] from a file's syntax tree, and resolves the types in it through the project index.
//!
//! The tree-sitter grammar puts top-level declarations side by side with their doc comments and annotations
//! (and a variable declaration is a run of loose tokens ending in `;`), so the top level is read as a
//! sequence. Class-like bodies are made of `class_member` nodes.

use super::types::parse_type;
use super::*;
use crate::index::SymbolIndex;
use crate::parser::dart_file::{
    declared_type, extract_directives, extract_part_directives, extract_part_of, has_child,
    is_named_group, optional_parameter_parts, text,
};
use crate::parser::dart_types::DirectiveKind;
use std::path::Path;
use tree_sitter::Node;

/// `dart:core` names (and the special types) that the index never resolves.
const DART_CORE: &[&str] = &[
    "ArgumentError",
    "AssertionError",
    "BidirectionalIterator",
    "BigInt",
    "bool",
    "Comparable",
    "ConcurrentModificationError",
    "DateTime",
    "Deprecated",
    "double",
    "Duration",
    "dynamic",
    "Enum",
    "Error",
    "Exception",
    "Expando",
    "Finalizer",
    "FormatException",
    "Function",
    "Future",
    "int",
    "IndexError",
    "Invocation",
    "Iterable",
    "Iterator",
    "List",
    "Map",
    "MapEntry",
    "Match",
    "Never",
    "NoSuchMethodError",
    "Null",
    "num",
    "Object",
    "OutOfMemoryError",
    "Pattern",
    "RangeError",
    "Record",
    "RegExp",
    "RegExpMatch",
    "RuneIterator",
    "Runes",
    "Set",
    "Sink",
    "StackOverflowError",
    "StackTrace",
    "StateError",
    "Stopwatch",
    "Stream",
    "String",
    "StringBuffer",
    "StringSink",
    "Symbol",
    "Type",
    "TypeError",
    "UnimplementedError",
    "UnsupportedError",
    "Uri",
    "UriData",
    "void",
    "WeakReference",
];

const CLASS_MODIFIERS: &[&str] = &["abstract", "sealed", "final", "base", "interface", "mixin"];

/// Builds the model of the file at `path` (under `root`) from its parsed `tree` and `content`. `index`
/// resolves type names; `package` is the package's name, for `package:` URIs.
pub fn library(
    root: &Path,
    package: &str,
    path: &Path,
    content: &str,
    tree: &tree_sitter::Tree,
    index: &SymbolIndex,
) -> Library {
    let root_node = tree.root_node();
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let uri = match relative_text.strip_prefix("lib/") {
        Some(rest) => format!("package:{package}/{rest}"),
        None => relative_text.clone(),
    };

    let directives = extract_directives(root_node, content);
    let directive = |d: &crate::parser::dart_types::Directive| Directive {
        uri: d.uri.clone(),
        prefix: d.prefix.clone(),
        show: d.show.clone(),
        hide: d.hide.clone(),
    };
    let builder = Builder {
        content,
        index,
        path,
        package,
    };
    let mut library = Library {
        uri,
        path: relative_text,
        imports: directives
            .iter()
            .filter(|d| d.kind == DirectiveKind::Import)
            .map(directive)
            .collect(),
        exports: directives
            .iter()
            .filter(|d| d.kind == DirectiveKind::Export)
            .map(directive)
            .collect(),
        parts: extract_part_directives(root_node, content),
        part_of: extract_part_of(root_node, content),
        classes: Vec::new(),
        enums: Vec::new(),
        mixins: Vec::new(),
        extensions: Vec::new(),
        extension_types: Vec::new(),
        typedefs: Vec::new(),
        functions: Vec::new(),
        getters: Vec::new(),
        variables: Vec::new(),
    };
    builder.top_level(root_node, &mut library);
    library
}

struct Builder<'a> {
    content: &'a str,
    index: &'a SymbolIndex,
    path: &'a Path,
    package: &'a str,
}

/// Doc comments and annotations seen before a declaration.
#[derive(Default)]
struct Pending<'t> {
    doc: Vec<String>,
    annotations: Vec<Node<'t>>,
    /// Loose tokens of a top-level variable declaration (`final`, `late`, the type…).
    tokens: Vec<Node<'t>>,
}

impl<'t> Pending<'t> {
    fn clear(&mut self) {
        self.doc.clear();
        self.annotations.clear();
        self.tokens.clear();
    }
}

fn line(node: Node) -> usize {
    node.start_position().row + 1
}

fn is_type_node(kind: &str) -> bool {
    matches!(
        kind,
        "type_identifier"
            | "type_arguments"
            | "function_type"
            | "record_type"
            | "void_type"
            | "?"
            | "."
    )
}

impl<'a> Builder<'a> {
    fn text(&self, node: Node) -> &'a str {
        text(node, self.content)
    }

    fn doc(&self, lines: &[String]) -> Option<String> {
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// The text of a `///` or `/** */` comment without its markers, or `None` for another comment.
    fn doc_lines(&self, comment: Node) -> Option<Vec<String>> {
        let raw = self.text(comment);
        if let Some(rest) = raw.strip_prefix("///") {
            return Some(vec![rest.strip_prefix(' ').unwrap_or(rest).to_string()]);
        }
        let body = raw.strip_prefix("/**")?.strip_suffix("*/")?;
        Some(
            body.lines()
                .map(|l| {
                    let l = l.trim();
                    let l = l.strip_prefix('*').unwrap_or(l);
                    l.strip_prefix(' ').unwrap_or(l).to_string()
                })
                .filter(|l| !l.is_empty())
                .collect(),
        )
    }

    // ----- top level -----

    fn top_level(&self, root: Node, library: &mut Library) {
        let mut pending = Pending::default();
        let mut cursor = root.walk();
        let children: Vec<Node> = root.children(&mut cursor).collect();
        let mut i = 0;
        while i < children.len() {
            let node = children[i];
            match node.kind() {
                "comment" | "documentation_comment" => {
                    if let Some(lines) = self.doc_lines(node) {
                        pending.doc.extend(lines);
                    }
                }
                "annotation" => pending.annotations.push(node),
                "class_declaration" => {
                    library.classes.push(self.class(node, &pending));
                    pending.clear();
                }
                "enum_declaration" => {
                    library.enums.push(self.enum_(node, &pending));
                    pending.clear();
                }
                "mixin_declaration" => {
                    library.mixins.push(self.mixin(node, &pending));
                    pending.clear();
                }
                "extension_declaration" => {
                    library.extensions.push(self.extension(node, &pending));
                    pending.clear();
                }
                "extension_type_declaration" => {
                    library
                        .extension_types
                        .push(self.extension_type(node, &pending));
                    pending.clear();
                }
                "type_alias" => {
                    library.typedefs.push(self.typedef(node, &pending));
                    pending.clear();
                }
                // A top-level setter parses as a function whose return type is `set`; the model has no
                // top-level setters, so it's skipped.
                "function_signature" if self.is_top_level_setter(node) => {
                    if children
                        .get(i + 1)
                        .is_some_and(|n| n.kind() == "function_body")
                    {
                        i += 1;
                    }
                    pending.clear();
                }
                "function_signature" => {
                    let body = children.get(i + 1).filter(|n| n.kind() == "function_body");
                    let is_external = pending.tokens.iter().any(|t| t.kind() == "external");
                    let method = self.method(node, *body.unwrap_or(&node), &pending, &[], false);
                    library.functions.push(Function {
                        name: method.name,
                        line: method.line,
                        doc: method.doc,
                        annotations: method.annotations,
                        return_type: method.return_type,
                        type_parameters: method.type_parameters,
                        params: method.params,
                        body_modifier: method.body_modifier,
                        is_external,
                    });
                    if body.is_some() {
                        i += 1;
                    }
                    pending.clear();
                }
                "getter_signature" => {
                    let body = children.get(i + 1).filter(|n| n.kind() == "function_body");
                    if let Some(getter) = self.getter(node, body.is_none(), false, &pending, &[]) {
                        library.getters.push(getter);
                    }
                    if body.is_some() {
                        i += 1;
                    }
                    pending.clear();
                }
                "setter_signature" => {
                    if children
                        .get(i + 1)
                        .is_some_and(|n| n.kind() == "function_body")
                    {
                        i += 1;
                    }
                    pending.clear();
                }
                "initialized_identifier_list" | "static_final_declaration_list" => {
                    let mut nodes = pending.tokens.clone();
                    nodes.push(node);
                    for field in self.fields(&nodes, &pending, false, &[]) {
                        library.variables.push(Variable {
                            name: field.name,
                            line: field.line,
                            doc: field.doc,
                            annotations: field.annotations,
                            ty: field.ty,
                            is_final: field.is_final,
                            is_const: field.is_const,
                            is_late: field.is_late,
                            initializer: field.initializer,
                        });
                    }
                    pending.clear();
                }
                ";" => pending.clear(),
                "import_or_export" | "library_name" | "part_directive" | "part_of_directive" => {
                    pending.clear()
                }
                _ => pending.tokens.push(node),
            }
            i += 1;
        }
    }

    fn is_top_level_setter(&self, signature: Node) -> bool {
        let mut cursor = signature.walk();
        let return_type: Vec<Node> = signature
            .children_by_field_name("return_type", &mut cursor)
            .collect();
        matches!(return_type.as_slice(), [only] if self.text(*only) == "set")
    }

    // ----- shared pieces -----

    fn annotations(&self, nodes: &[Node]) -> Vec<Annotation> {
        nodes.iter().filter_map(|n| self.annotation(*n)).collect()
    }

    fn annotation(&self, node: Node) -> Option<Annotation> {
        let mut cursor = node.walk();
        let mut names = Vec::new();
        let mut arguments = None;
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "type_identifier" => names.push(self.text(child).to_string()),
                "scoped_identifier" | "qualified" => {
                    names.extend(self.text(child).split('.').map(str::to_string))
                }
                "annotation_arguments" | "arguments" => {
                    arguments = Some(self.arguments(child));
                }
                _ => {}
            }
        }
        // `@a.B` is an import prefix and a name, or a name and a named constructor (`@Foo.named()`). Syntax
        // can't tell them apart; Dart's naming convention can (prefixes are lower case, types upper case).
        let is_type = |s: &str| s.trim_start_matches('_').starts_with(char::is_uppercase);
        let (prefix, name, constructor) = match names.as_slice() {
            [name] => (None, name.clone(), None),
            [first, second] if is_type(first) => (None, first.clone(), Some(second.clone())),
            [first, second] => (Some(first.clone()), second.clone(), None),
            [prefix, name, constructor, ..] => (
                Some(prefix.clone()),
                name.clone(),
                Some(constructor.clone()),
            ),
            [] => return None,
        };
        Some(Annotation {
            name,
            prefix,
            constructor,
            arguments,
            line: line(node),
        })
    }

    fn arguments(&self, node: Node) -> Arguments {
        let mut result = Arguments::default();
        let mut cursor = node.walk();
        for argument in node
            .children(&mut cursor)
            .filter(|n| n.kind() == "argument")
        {
            let mut c = argument.walk();
            let children: Vec<Node> = argument.named_children(&mut c).collect();
            let (Some(inner), Some(last)) = (children.first().copied(), children.last().copied())
            else {
                continue;
            };
            if inner.kind() == "named_argument" {
                let mut c = inner.walk();
                let parts: Vec<Node> = inner.named_children(&mut c).collect();
                let Some(label) = parts.first() else { continue };
                let name = self.text(*label).trim_end_matches(':').trim().to_string();
                if let Some(first) = parts.get(1) {
                    let last = parts[parts.len() - 1];
                    result.named.insert(name, self.expr_span(*first, last));
                }
            } else {
                // `Mood.calm` is two sibling nodes, `Mood` and `.calm`.
                result.positional.push(self.expr_span(inner, last));
            }
        }
        result
    }

    /// An expression that the grammar splits into several sibling nodes (`Foo` `.bar`).
    fn expr_span(&self, first: Node, last: Node) -> Expr {
        let source = self.content[first.start_byte()..last.end_byte()].to_string();
        let literal = if first.id() == last.id() {
            self.literal(first)
        } else {
            None
        };
        Expr { source, literal }
    }

    fn literal(&self, node: Node) -> Option<Literal> {
        let raw = self.text(node);
        match node.kind() {
            "decimal_integer_literal" => raw.replace('_', "").parse().ok().map(Literal::Int),
            "hex_integer_literal" => {
                let digits = raw.trim_start_matches("0x").trim_start_matches("0X");
                i64::from_str_radix(&digits.replace('_', ""), 16)
                    .ok()
                    .map(Literal::Int)
            }
            "decimal_floating_point_literal" => {
                raw.replace('_', "").parse().ok().map(Literal::Double)
            }
            "true" => Some(Literal::Bool(true)),
            "false" => Some(Literal::Bool(false)),
            "null_literal" => Some(Literal::Null),
            "string_literal" => self.string_value(node).map(Literal::String),
            "list_literal" => {
                let mut cursor = node.walk();
                node.named_children(&mut cursor)
                    .filter(|n| n.kind() != "const_builtin" && self.text(*n) != "const")
                    .filter(|n| n.kind() != "type_arguments")
                    .map(|n| self.literal(n))
                    .collect::<Option<Vec<_>>>()
                    .map(Literal::List)
            }
            "set_or_map_literal" => {
                let mut cursor = node.walk();
                let entries: Vec<Node> = node
                    .named_children(&mut cursor)
                    .filter(|n| n.kind() != "type_arguments" && self.text(*n) != "const")
                    .collect();
                if entries.is_empty() {
                    return Some(Literal::Map(Vec::new()));
                }
                entries
                    .iter()
                    .map(|pair| {
                        if pair.kind() != "pair" {
                            return None;
                        }
                        Some(MapEntry {
                            key: self.literal(pair.child_by_field_name("key")?)?,
                            value: self.literal(pair.child_by_field_name("value")?)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(Literal::Map)
            }
            "unary_expression" if raw.starts_with('-') => {
                let mut cursor = node.walk();
                let operand = node.named_children(&mut cursor).last()?;
                match self.literal(operand)? {
                    Literal::Int(v) => Some(Literal::Int(-v)),
                    Literal::Double(v) => Some(Literal::Double(-v)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The value of a string literal without interpolation or escapes (adjacent strings are joined);
    /// `None` otherwise.
    fn string_value(&self, node: Node) -> Option<String> {
        let mut value = String::new();
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            let kind = current.kind();
            if kind.starts_with("template_chars") {
                value.insert_str(0, self.text(current));
                continue;
            }
            if kind.contains("substitution") || kind.contains("escape") {
                return None;
            }
            let mut cursor = current.walk();
            // Pushed in order and popped in reverse, so the text is prepended.
            for child in current.named_children(&mut cursor) {
                stack.push(child);
            }
        }
        Some(value)
    }

    fn parse_type(&self, source: &str, type_parameters: &[String]) -> Option<Type> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        let mut parsed = parse_type(source);
        self.resolve(&mut parsed, type_parameters);
        Some(parsed)
    }

    /// The type written by `nodes` (from the first node's start to the last node's end).
    fn type_of(&self, nodes: &[Node], type_parameters: &[String]) -> Option<Type> {
        let first = nodes.first()?;
        let last = nodes.last()?;
        self.parse_type(
            &self.content[first.start_byte()..last.end_byte()],
            type_parameters,
        )
    }

    fn resolve(&self, ty: &mut Type, type_parameters: &[String]) {
        for argument in &mut ty.arguments {
            self.resolve(argument, type_parameters);
        }
        if let Some(function) = &mut ty.function {
            let mut scope = type_parameters.to_vec();
            scope.extend(function.type_parameters.iter().cloned());
            if let Some(return_type) = &mut function.return_type {
                self.resolve(return_type, &scope);
            }
            for t in &mut function.positional {
                self.resolve(t, &scope);
            }
            for named in &mut function.named {
                self.resolve(&mut named.ty, &scope);
            }
            return;
        }
        if let Some(record) = &mut ty.record {
            for t in &mut record.positional {
                self.resolve(t, type_parameters);
            }
            for named in &mut record.named {
                self.resolve(&mut named.ty, type_parameters);
            }
            return;
        }
        // Type parameters, then the package's own declarations (which shadow `dart:core`), then `dart:core`.
        let kind_and_library = if ty.prefix.is_none() && type_parameters.contains(&ty.name) {
            (ResolvedKind::TypeParameter, None)
        } else {
            let written = match &ty.prefix {
                Some(prefix) => format!("{prefix}.{}", ty.name),
                None => ty.name.clone(),
            };
            match self.index.resolve(self.path, &written) {
                Ok(Some(found)) => {
                    let kind = match found.kind {
                        "enum" => ResolvedKind::Enum,
                        "mixin" => ResolvedKind::Mixin,
                        "type_alias" => ResolvedKind::Typedef,
                        "extension_type" => ResolvedKind::ExtensionType,
                        _ => ResolvedKind::Class,
                    };
                    (kind, found.file.map(|file| self.uri_of(&file)))
                }
                Ok(None) if ty.prefix.is_none() && DART_CORE.contains(&ty.name.as_str()) => {
                    (ResolvedKind::DartCore, None)
                }
                // An ambiguous name is reported by the build that uses it; here it resolves to nothing.
                Ok(None) | Err(_) => (ResolvedKind::Unresolved, None),
            }
        };
        ty.resolved = Some(Resolved {
            kind: kind_and_library.0,
            library: kind_and_library.1,
        });
    }

    fn uri_of(&self, relative: &str) -> String {
        match relative.strip_prefix("lib/") {
            Some(rest) => format!("package:{}/{rest}", self.package),
            None => relative.to_string(),
        }
    }

    fn type_parameters(&self, node: Option<Node>, outer: &[String]) -> Vec<TypeParameter> {
        let Some(node) = node else {
            return Vec::new();
        };
        let mut cursor = node.walk();
        let params: Vec<Node> = node
            .children(&mut cursor)
            .filter(|n| n.kind() == "type_parameter")
            .collect();
        let mut names: Vec<String> = outer.to_vec();
        names.extend(
            params
                .iter()
                .filter_map(|p| p.child_by_field_name("name"))
                .map(|n| self.text(n).to_string()),
        );
        params
            .iter()
            .filter_map(|p| {
                let name = self.text(p.child_by_field_name("name")?).to_string();
                let mut c = p.walk();
                let bound: Vec<Node> = p.children_by_field_name("bound", &mut c).collect();
                Some(TypeParameter {
                    name,
                    bound: self.type_of(&bound, &names),
                })
            })
            .collect()
    }

    /// The comma-separated types among `node`'s children after the keyword `after` (`with`, `on`,
    /// `implements`), up to the next child that isn't part of a type.
    fn type_list(&self, node: Node, after: &str, scope: &[String]) -> Vec<Type> {
        let mut cursor = node.walk();
        let mut started = false;
        let mut current: Vec<Node> = Vec::new();
        let mut types = Vec::new();
        for child in node.children(&mut cursor) {
            if !started {
                started = child.kind() == after;
                continue;
            }
            match child.kind() {
                kind if is_type_node(kind) => current.push(child),
                "," => {
                    types.extend(self.type_of(&current, scope));
                    current.clear();
                }
                _ => break,
            }
        }
        types.extend(self.type_of(&current, scope));
        types
    }

    // ----- declarations -----

    fn class(&self, node: Node, pending: &Pending) -> Class {
        let mut annotation_nodes = pending.annotations.clone();
        let mut cursor = node.walk();
        let mut modifiers = Vec::new();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "annotation" => annotation_nodes.push(child),
                kind if CLASS_MODIFIERS.contains(&kind) => modifiers.push(kind.to_string()),
                _ => {}
            }
        }
        let name_node = node.child_by_field_name("name");
        let type_parameters =
            self.type_parameters(node.child_by_field_name("type_parameters"), &[]);
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        let (superclass, mixins) = match node.child_by_field_name("superclass") {
            Some(superclass) => {
                let mut c = superclass.walk();
                let ty: Vec<Node> = superclass.children_by_field_name("type", &mut c).collect();
                let mut c = superclass.walk();
                let mixins = superclass
                    .children(&mut c)
                    .find(|n| n.kind() == "mixins")
                    .map(|m| self.type_list(m, "with", &scope))
                    .unwrap_or_default();
                (self.type_of(&ty, &scope), mixins)
            }
            None => (None, Vec::new()),
        };
        let interfaces = node
            .child_by_field_name("interfaces")
            .map(|i| self.type_list(i, "implements", &scope))
            .unwrap_or_default();
        let name = name_node.map_or_else(
            || self.mixin_application_name(node),
            |n| self.text(n).to_string(),
        );
        Class {
            members: node
                .child_by_field_name("body")
                .map(|body| self.members(body, &scope))
                .unwrap_or_default(),
            name,
            line: name_node.map_or(line(node), line),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            modifiers,
            type_parameters,
            superclass,
            mixins,
            interfaces,
        }
    }

    /// `class M = Object with Mx;` keeps its name inside `mixin_application_class`.
    fn mixin_application_name(&self, node: Node) -> String {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .find(|n| n.kind() == "mixin_application_class")
            .and_then(|application| {
                let mut c = application.walk();
                application
                    .named_children(&mut c)
                    .find(|n| n.kind() == "identifier")
                    .map(|n| self.text(n).to_string())
            })
            .unwrap_or_default()
    }

    fn enum_(&self, node: Node, pending: &Pending) -> Enum {
        let mut annotation_nodes = pending.annotations.clone();
        let mut cursor = node.walk();
        let mut mixins = Vec::new();
        let mut interfaces = Vec::new();
        let type_parameters =
            self.type_parameters(node.child_by_field_name("type_parameters"), &[]);
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "annotation" => annotation_nodes.push(child),
                "mixins" => mixins = self.type_list(child, "with", &scope),
                "interfaces" => interfaces = self.type_list(child, "implements", &scope),
                _ => {}
            }
        }
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        let mut values = Vec::new();
        let mut members = Members::default();
        if let Some(body) = node.child_by_field_name("body") {
            let mut doc = Vec::new();
            let mut c = body.walk();
            for child in body.children(&mut c) {
                match child.kind() {
                    "comment" | "documentation_comment" => {
                        doc.extend(self.doc_lines(child).unwrap_or_default())
                    }
                    "enum_constant" => {
                        let mut cc = child.walk();
                        let annotations: Vec<Node> = child
                            .children(&mut cc)
                            .filter(|n| n.kind() == "annotation")
                            .collect();
                        let mut cc = child.walk();
                        let arguments = child
                            .children(&mut cc)
                            .find(|n| n.kind() == "argument_part")
                            .and_then(|part| {
                                let mut p = part.walk();
                                part.children(&mut p).find(|n| n.kind() == "arguments")
                            })
                            .map(|a| self.arguments(a));
                        let name_node = child.child_by_field_name("name");
                        values.push(EnumValue {
                            name: name_node
                                .map(|n| self.text(n).to_string())
                                .unwrap_or_default(),
                            line: name_node.map_or(line(child), line),
                            doc: self.doc(&doc),
                            annotations: self.annotations(&annotations),
                            arguments,
                        });
                        doc.clear();
                    }
                    _ => {}
                }
            }
            members = self.members(body, &scope);
        }
        Enum {
            name,
            line: name_node.map_or(line(node), line),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            type_parameters,
            mixins,
            interfaces,
            values,
            members,
        }
    }

    fn mixin(&self, node: Node, pending: &Pending) -> Mixin {
        let mut annotation_nodes = pending.annotations.clone();
        let mut modifiers = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "annotation" => annotation_nodes.push(child),
                "base" => modifiers.push("base".to_string()),
                _ => {}
            }
        }
        let name_node = node.child_by_field_name("name");
        let name = name_node
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        let type_parameters = self.type_parameters(
            {
                let mut c = node.walk();
                node.children(&mut c)
                    .find(|n| n.kind() == "type_parameters")
            },
            &[],
        );
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        Mixin {
            on: self.type_list(node, "on", &scope),
            interfaces: node
                .child_by_field_name("interfaces")
                .map(|i| self.type_list(i, "implements", &scope))
                .unwrap_or_default(),
            members: node
                .child_by_field_name("body")
                .map(|body| self.members(body, &scope))
                .unwrap_or_default(),
            name,
            line: name_node.map_or(line(node), line),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            modifiers,
            type_parameters,
        }
    }

    fn extension(&self, node: Node, pending: &Pending) -> Extension {
        let mut annotation_nodes = pending.annotations.clone();
        let mut cursor = node.walk();
        annotation_nodes.extend(
            node.children(&mut cursor)
                .filter(|n| n.kind() == "annotation"),
        );
        let name_node = node.child_by_field_name("name");
        let type_parameters =
            self.type_parameters(node.child_by_field_name("type_parameters"), &[]);
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        let mut c = node.walk();
        let on: Vec<Node> = node.children_by_field_name("class", &mut c).collect();
        let name = name_node.map(|n| self.text(n).to_string());
        Extension {
            members: node
                .child_by_field_name("body")
                .map(|body| self.members(body, &scope))
                .unwrap_or_default(),
            name,
            line: name_node.map_or(line(node), line),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            on: self.type_of(&on, &scope),
            type_parameters,
        }
    }

    fn extension_type(&self, node: Node, pending: &Pending) -> ExtensionType {
        let mut annotation_nodes = pending.annotations.clone();
        let mut cursor = node.walk();
        annotation_nodes.extend(
            node.children(&mut cursor)
                .filter(|n| n.kind() == "annotation"),
        );
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).trim().to_string())
            .unwrap_or_default();
        let mut c = node.walk();
        let type_parameters = self.type_parameters(
            node.children(&mut c)
                .find(|n| n.kind() == "type_parameters"),
            &[],
        );
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        let representation = node.child_by_field_name("representation");
        let (representation_name, representation_type) = match representation {
            Some(rep) => {
                let mut c = rep.walk();
                let ty: Vec<Node> = rep.children_by_field_name("type", &mut c).collect();
                (
                    rep.child_by_field_name("name")
                        .map(|n| self.text(n).to_string())
                        .unwrap_or_default(),
                    self.type_of(&ty, &scope),
                )
            }
            None => (String::new(), None),
        };
        ExtensionType {
            interfaces: self.type_list(node, "implements", &scope),
            members: node
                .child_by_field_name("body")
                .map(|body| self.members(body, &scope))
                .unwrap_or_default(),
            line: node.child_by_field_name("name").map_or(line(node), line),
            name,
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            type_parameters,
            representation_name,
            representation_type,
        }
    }

    fn typedef(&self, node: Node, pending: &Pending) -> Typedef {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        let type_parameters = self.type_parameters(
            children
                .iter()
                .copied()
                .find(|n| n.kind() == "type_parameters"),
            &[],
        );
        let scope: Vec<String> = type_parameters.iter().map(|t| t.name.clone()).collect();
        let equals = children.iter().position(|n| n.kind() == "=");
        let (name, aliased) = match equals {
            // `typedef Json = Map<String, dynamic>;`
            Some(equals) => {
                let name = children[..equals]
                    .iter()
                    .find(|n| n.kind() == "type_identifier")
                    .map(|n| self.text(*n).to_string())
                    .unwrap_or_default();
                let aliased: Vec<Node> = children[equals + 1..]
                    .iter()
                    .copied()
                    .filter(|n| n.kind() != ";")
                    .collect();
                (name, self.type_of(&aliased, &scope))
            }
            // `typedef void Callback(int x);`: the name is the last type name before the parameters.
            None => {
                let params = children
                    .iter()
                    .position(|n| n.kind() == "formal_parameter_list")
                    .unwrap_or(children.len());
                let name_position = children[..params]
                    .iter()
                    .rposition(|n| n.kind() == "type_identifier");
                let name = name_position
                    .map(|i| self.text(children[i]).to_string())
                    .unwrap_or_default();
                let return_nodes: Vec<Node> = name_position
                    .map(|i| {
                        children[1..i]
                            .iter()
                            .copied()
                            .filter(|n| is_type_node(n.kind()))
                            .collect()
                    })
                    .unwrap_or_default();
                let return_text = return_nodes
                    .first()
                    .zip(return_nodes.last())
                    .map(|(a, b)| &self.content[a.start_byte()..b.end_byte()])
                    .unwrap_or("dynamic");
                let params_text = children.get(params).map(|p| self.text(*p)).unwrap_or("()");
                (
                    name,
                    self.parse_type(&format!("{return_text} Function{params_text}"), &scope),
                )
            }
        };
        let mut annotation_nodes = pending.annotations.clone();
        annotation_nodes.extend(
            children
                .iter()
                .copied()
                .filter(|n| n.kind() == "annotation"),
        );
        Typedef {
            name,
            line: line(node),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&annotation_nodes),
            type_parameters,
            aliased,
        }
    }

    // ----- members -----

    fn members(&self, body: Node, scope: &[String]) -> Members {
        let mut members = Members::default();
        let mut doc: Vec<String> = Vec::new();
        let mut cursor = body.walk();
        for member in body.children(&mut cursor) {
            match member.kind() {
                "comment" | "documentation_comment" => {
                    doc.extend(self.doc_lines(member).unwrap_or_default());
                    continue;
                }
                "class_member" => {}
                // Enum constants take their own doc comments (see `enum_`).
                _ => {
                    doc.clear();
                    continue;
                }
            }
            let mut c = member.walk();
            let children: Vec<Node> = member.children(&mut c).collect();
            let pending = Pending {
                doc: std::mem::take(&mut doc),
                annotations: children
                    .iter()
                    .copied()
                    .filter(|n| n.kind() == "annotation")
                    .collect(),
                tokens: Vec::new(),
            };
            let body_node = children
                .iter()
                .copied()
                .find(|n| n.kind() == "function_body");
            for node in children
                .iter()
                .copied()
                .filter(|n| matches!(n.kind(), "declaration" | "method_signature"))
            {
                self.member(node, body_node, &pending, scope, &mut members);
            }
        }
        members
    }

    fn member(
        &self,
        node: Node,
        body: Option<Node>,
        pending: &Pending,
        scope: &[String],
        members: &mut Members,
    ) {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        let signature = children.iter().copied().find(|n| {
            matches!(
                n.kind(),
                "constructor_signature"
                    | "constant_constructor_signature"
                    | "factory_constructor_signature"
                    | "redirecting_factory_constructor_signature"
                    | "getter_signature"
                    | "setter_signature"
                    | "function_signature"
                    | "operator_signature"
            )
        });
        // `static set x(…)` puts `static` inside the setter's signature.
        let is_static = has_child(node, "static")
            || signature.is_some_and(|s| {
                s.kind() == "setter_signature" && self.text(s).starts_with("static")
            });
        let is_abstract = body.is_none();
        match signature {
            Some(s) if s.kind() == "getter_signature" => {
                members
                    .getters
                    .extend(self.getter(s, is_abstract, is_static, pending, scope));
            }
            Some(s) if s.kind() == "setter_signature" => {
                let parameter_type = s
                    .child_by_field_name("parameters")
                    .and_then(|list| {
                        let mut lc = list.walk();
                        list.children(&mut lc)
                            .find(|n| n.kind() == "formal_parameter")
                    })
                    .and_then(|p| {
                        let (text, nullable) = declared_type(p, self.content);
                        self.parse_type(
                            &format!("{text}{}", if nullable { "?" } else { "" }),
                            scope,
                        )
                    });
                members.setters.push(Setter {
                    name: s
                        .child_by_field_name("name")
                        .map(|n| self.text(n).to_string())
                        .unwrap_or_default(),
                    line: line(s),
                    doc: self.doc(&pending.doc),
                    annotations: self.annotations(&pending.annotations),
                    ty: parameter_type,
                    is_static,
                });
            }
            Some(s) if matches!(s.kind(), "function_signature" | "operator_signature") => {
                let body = body.unwrap_or(s);
                members
                    .methods
                    .push(self.method(s, body, pending, scope, is_static));
                if let Some(last) = members.methods.last_mut() {
                    last.is_abstract = is_abstract;
                }
            }
            Some(s) => members
                .constructors
                .push(self.constructor(s, pending, scope)),
            None => members
                .fields
                .extend(self.fields(&children, pending, is_static, scope)),
        }
    }

    /// Fields or variables from a declaration's children: modifiers, the type, and a list of variables.
    fn fields(
        &self,
        nodes: &[Node],
        pending: &Pending,
        is_static: bool,
        scope: &[String],
    ) -> Vec<Field> {
        let has = |kind: &str| nodes.iter().any(|n| n.kind() == kind);
        let type_nodes: Vec<Node> = nodes
            .iter()
            .copied()
            .filter(|n| is_type_node(n.kind()))
            .collect();
        let ty = self.type_of(&type_nodes, scope);
        let mut fields = Vec::new();
        for list in nodes.iter().filter(|n| {
            matches!(
                n.kind(),
                "initialized_identifier_list" | "static_final_declaration_list" | "identifier_list"
            )
        }) {
            let mut c = list.walk();
            for variable in list.named_children(&mut c) {
                let name_node = variable
                    .child_by_field_name("name")
                    .or_else(|| (variable.kind() == "identifier").then_some(variable));
                let Some(name_node) = name_node else { continue };
                let name = self.text(name_node).to_string();
                fields.push(Field {
                    is_private: name.starts_with('_'),
                    name,
                    line: line(name_node),
                    doc: self.doc(&pending.doc),
                    annotations: self.annotations(&pending.annotations),
                    ty: ty.clone(),
                    is_static,
                    is_final: has("final"),
                    is_const: has("const"),
                    is_late: has("late"),
                    initializer: {
                        // Like defaults, an initialiser can be several sibling nodes (`Foo.bar`).
                        let mut vc = variable.walk();
                        let value: Vec<Node> =
                            variable.children_by_field_name("value", &mut vc).collect();
                        match (value.first(), value.last()) {
                            (Some(first), Some(last)) => Some(self.expr_span(*first, *last)),
                            _ => None,
                        }
                    },
                });
            }
        }
        fields
    }

    fn getter(
        &self,
        signature: Node,
        is_abstract: bool,
        is_static: bool,
        pending: &Pending,
        scope: &[String],
    ) -> Option<Getter> {
        let name = self
            .text(signature.child_by_field_name("name")?)
            .to_string();
        let mut c = signature.walk();
        let return_nodes: Vec<Node> = signature
            .children_by_field_name("return_type", &mut c)
            .collect();
        Some(Getter {
            name,
            line: line(signature),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&pending.annotations),
            ty: self.type_of(&return_nodes, scope),
            is_static,
            is_abstract,
        })
    }

    fn method(
        &self,
        signature: Node,
        body: Node,
        pending: &Pending,
        outer: &[String],
        is_static: bool,
    ) -> Method {
        let mut c = signature.walk();
        let children: Vec<Node> = signature.children(&mut c).collect();
        let type_parameters = self.type_parameters(
            children
                .iter()
                .copied()
                .find(|n| n.kind() == "type_parameters"),
            outer,
        );
        let mut scope = outer.to_vec();
        scope.extend(type_parameters.iter().map(|t| t.name.clone()));
        let mut c = signature.walk();
        let return_nodes: Vec<Node> = signature
            .children_by_field_name("return_type", &mut c)
            .collect();
        let is_operator = signature.kind() == "operator_signature";
        let name = if is_operator {
            signature
                .child_by_field_name("operator")
                .map(|n| self.text(n).to_string())
        } else {
            signature
                .child_by_field_name("name")
                .map(|n| self.text(n).to_string())
        }
        .unwrap_or_default();
        let params = children
            .iter()
            .copied()
            .find(|n| n.kind() == "formal_parameter_list")
            .map(|list| self.parameters(list, &scope))
            .unwrap_or_default();
        Method {
            name,
            line: line(signature),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&pending.annotations),
            return_type: self.type_of(&return_nodes, &scope),
            type_parameters,
            params,
            is_static,
            is_abstract: false,
            is_operator,
            body_modifier: self.body_modifier(body),
        }
    }

    /// `async`, `async*` or `sync*` at the start of a function body.
    fn body_modifier(&self, body: Node) -> Option<String> {
        if body.kind() != "function_body" {
            return None;
        }
        let mut cursor = body.walk();
        let mut modifier = String::new();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "async" | "sync" | "*" => modifier.push_str(self.text(child)),
                _ => break,
            }
        }
        (!modifier.is_empty()).then_some(modifier)
    }

    fn constructor(&self, signature: Node, pending: &Pending, scope: &[String]) -> Constructor {
        let mut c = signature.walk();
        let names: Vec<&str> = signature
            .children_by_field_name("name", &mut c)
            .filter(|n| n.kind() == "identifier")
            .map(|n| self.text(n))
            .collect();
        let name = names.get(1).map(|n| n.to_string());
        let kind = match signature.kind() {
            "redirecting_factory_constructor_signature" => ConstructorKind::RedirectingFactory,
            "factory_constructor_signature" => ConstructorKind::Factory,
            _ => ConstructorKind::Generative,
        };
        let mut c = signature.walk();
        let target: Vec<Node> = signature.children_by_field_name("target", &mut c).collect();
        Constructor {
            name,
            line: line(signature),
            doc: self.doc(&pending.doc),
            annotations: self.annotations(&pending.annotations),
            kind,
            is_const: has_child(signature, "const"),
            redirects_to: self.type_of(&target, scope),
            params: signature
                .child_by_field_name("parameters")
                .map(|list| self.parameters(list, scope))
                .unwrap_or_default(),
        }
    }

    fn parameters(&self, list: Node, scope: &[String]) -> Vec<Parameter> {
        let mut params = Vec::new();
        let mut cursor = list.walk();
        for child in list.children(&mut cursor) {
            match child.kind() {
                "formal_parameter" => {
                    params.extend(self.parameter(
                        child,
                        ParameterKind::Positional,
                        false,
                        &[],
                        scope,
                    ));
                }
                "optional_formal_parameters" => {
                    let kind = if is_named_group(child) {
                        ParameterKind::Named
                    } else {
                        ParameterKind::OptionalPositional
                    };
                    for part in optional_parameter_parts(child) {
                        let Some(mut parameter) = self.parameter(
                            part.formal,
                            kind,
                            part.required,
                            &part.annotations,
                            scope,
                        ) else {
                            continue;
                        };
                        parameter.default = part
                            .default
                            .map(|(first, last)| self.expr_span(first, last));
                        params.push(parameter);
                    }
                }
                _ => {}
            }
        }
        params
    }

    fn parameter(
        &self,
        node: Node,
        kind: ParameterKind,
        required: bool,
        sibling_annotations: &[Node],
        scope: &[String],
    ) -> Option<Parameter> {
        let mut annotation_nodes = sibling_annotations.to_vec();
        let mut cursor = node.walk();
        annotation_nodes.extend(
            node.children(&mut cursor)
                .filter(|n| n.kind() == "annotation"),
        );
        let mut cursor = node.walk();
        let initializer = node
            .named_children(&mut cursor)
            .find(|n| matches!(n.kind(), "constructor_param" | "super_formal_parameter"));
        let (name, initializes, type_text, nullable, required_keyword) = match initializer {
            Some(inner) => {
                let mut c = inner.walk();
                let name = inner
                    .children(&mut c)
                    .filter(|n| n.kind() == "identifier")
                    .last()?;
                let (type_text, nullable) = declared_type(inner, self.content);
                let mut c = inner.walk();
                let required_keyword = inner.children(&mut c).any(|n| self.text(n) == "required");
                let initializes = if inner.kind() == "super_formal_parameter" {
                    Initializes::Super
                } else {
                    Initializes::This
                };
                (
                    self.text(name),
                    initializes,
                    type_text,
                    nullable,
                    required_keyword,
                )
            }
            None => {
                let name = node.child_by_field_name("name")?;
                let (type_text, nullable) = declared_type(node, self.content);
                (
                    self.text(name),
                    Initializes::Plain,
                    type_text,
                    nullable,
                    false,
                )
            }
        };
        let ty = if type_text.is_empty() {
            None
        } else {
            self.parse_type(
                &format!("{type_text}{}", if nullable { "?" } else { "" }),
                scope,
            )
        };
        Some(Parameter {
            name: name.to_string(),
            kind,
            required: kind == ParameterKind::Positional || required || required_keyword,
            default: None,
            initializes,
            ty,
            annotations: self.annotations(&annotation_nodes),
        })
    }
}
