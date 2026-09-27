use serde::Serialize;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone, Default, Serialize)]
pub struct ParsedFile {
    pub classes: Vec<DartClass>,
    pub enums: Vec<DartEnum>,
    /// URIs of the file's `part '...';` directives, without quotes.
    pub part_directives: Vec<String>,
    /// The library this file is a part of (`part of '...';`), if it is a part file.
    pub part_of: Option<String>,
    /// `import` and `export` directives, in source order (spec 0005).
    pub directives: Vec<Directive>,
    /// Every top-level type declaration, annotated or not (spec 0005).
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DirectiveKind {
    Import,
    Export,
}

/// `import 'src/money.dart' as m show Money;` →
/// `{ kind: Import, uri: "src/money.dart", prefix: Some("m"), show: ["Money"], hide: [] }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Directive {
    pub kind: DirectiveKind,
    pub uri: String,
    pub prefix: Option<String>,
    pub show: Vec<String>,
    pub hide: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeclarationKind {
    Class,
    Enum,
    Mixin,
    TypeAlias,
    ExtensionType,
}

/// A top-level type declared in a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Declaration {
    pub name: String,
    pub kind: DeclarationKind,
    /// Classes only: declares a `fromJson` constructor or factory.
    pub has_from_json: bool,
    /// Classes only: declares a `toJson` method.
    pub has_to_json: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TypeKind {
    String,
    Int,
    Double,
    Bool,
    DateTime,
    List(Box<DartType>),
    Map(Box<DartType>, Box<DartType>),
    // dart:core types json_serializable supports (spec 0005 step 3).
    Num,
    Dynamic,
    Object,
    Uri,
    BigInt,
    Duration,
    Set(Box<DartType>),
    Iterable(Box<DartType>),
    /// A named type Flint doesn't know, possibly prefixed (`Money`, `m.Money`).
    Custom(String),
    /// A type Flint can't serialize (records, function types) or a field with no declared type (empty).
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DartType {
    pub kind: TypeKind,
    pub is_nullable: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct DartField {
    pub name: String,
    /// 1-based line of the field declaration, for diagnostics.
    pub line: usize,
    pub dart_type: DartType,
    pub is_final: bool,
    pub from_json_expr: Option<String>,
    pub to_json_expr: Option<String>,
    pub metadata: HashMap<String, String>,
    pub converter: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DartClass {
    pub name: String,
    pub fields: Vec<DartField>,
    pub metadata: HashMap<String, String>,
    pub type_parameters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DartEnumValue {
    pub name: String,
    /// The JSON value, taken from the first of the plugin's `variant_annotations` on this constant
    /// (see `generators::select_variant_values`). The parser leaves it empty.
    pub value: Option<String>,
    /// The same value as Dart source (`1`, `true`, `"it's"`), so it can be emitted with its type intact.
    pub literal: Option<String>,
    pub annotations: Vec<DartEnumValueAnnotation>,
}

/// An annotation on an enum constant:
/// `@JsonValue('x')` → `{ name: "JsonValue", value: Some("x"), literal: Some("'x'") }`.
#[derive(Debug, Clone, Serialize)]
pub struct DartEnumValueAnnotation {
    pub name: String,
    /// The first argument with one pair of string quotes removed, if there is an argument.
    pub value: Option<String>,
    /// The first argument exactly as written in the source.
    pub literal: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DartEnum {
    pub name: String,
    pub annotations: Vec<String>,
    pub values: Vec<DartEnumValue>,
}

impl DartType {
    /// Every non-core type name in this type, including type arguments, in order of appearance:
    /// `Map<String, List<m.Money>>` → `m.Money`, and `Page<User?>` (a `Custom` with arguments) → `Page`, `User`.
    pub fn custom_names(&self) -> Vec<&str> {
        match &self.kind {
            TypeKind::Custom(name) => {
                let mut names = Vec::new();
                names_in_type_text(name, &mut names);
                names
            }
            TypeKind::List(inner) | TypeKind::Set(inner) | TypeKind::Iterable(inner) => {
                inner.custom_names()
            }
            TypeKind::Map(key, value) => {
                let mut names = key.custom_names();
                names.extend(value.custom_names());
                names
            }
            _ => Vec::new(),
        }
    }
}

/// Names that are always `dart:core` types, so they never need resolving inside type arguments.
const CORE_TYPE_NAMES: [&str; 18] = [
    "String", "int", "double", "bool", "num", "dynamic", "Object", "DateTime", "Uri", "BigInt",
    "Duration", "List", "Map", "Set", "Iterable", "void", "Null", "Never",
];

/// Collects the type names in type text such as `Page<Map<String, User?>>` (→ `Page`, `User`).
fn names_in_type_text<'a>(text: &'a str, names: &mut Vec<&'a str>) {
    let text = text.trim().trim_end_matches('?').trim_end();
    let (base, arguments) = match text.find('<') {
        Some(open) if text.ends_with('>') => (&text[..open], Some(&text[open + 1..text.len() - 1])),
        _ => (text, None),
    };
    let base = base.trim();
    let is_type_name = !base.is_empty()
        && !base.starts_with('(')
        && !base.contains("Function")
        && !CORE_TYPE_NAMES.contains(&base);
    if is_type_name {
        names.push(base);
    }
    if let Some(arguments) = arguments {
        for argument in split_top_level_commas(arguments) {
            names_in_type_text(argument, names);
        }
    }
}

/// Splits `A, B<C, D>, E` at the commas that aren't nested in `<>`, `()` or `{}`.
fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in text.char_indices() {
        match c {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

impl Display for DartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            TypeKind::String => write!(f, "String"),
            TypeKind::Int => write!(f, "int"),
            TypeKind::Double => write!(f, "double"),
            TypeKind::Bool => write!(f, "bool"),
            TypeKind::DateTime => write!(f, "DateTime"),
            TypeKind::List(inner) => write!(f, "List<{}>", inner),
            TypeKind::Map(key, value) => write!(f, "Map<{}, {}>", key, value),
            TypeKind::Num => write!(f, "num"),
            TypeKind::Dynamic => write!(f, "dynamic"),
            TypeKind::Object => write!(f, "Object"),
            TypeKind::Uri => write!(f, "Uri"),
            TypeKind::BigInt => write!(f, "BigInt"),
            TypeKind::Duration => write!(f, "Duration"),
            TypeKind::Set(inner) => write!(f, "Set<{}>", inner),
            TypeKind::Iterable(inner) => write!(f, "Iterable<{}>", inner),
            TypeKind::Custom(name) | TypeKind::Unsupported(name) => write!(f, "{}", name),
        }?;

        if self.is_nullable {
            write!(f, "?")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dart_type_display() {
        let t_str = DartType {
            kind: TypeKind::String,
            is_nullable: false,
        };
        assert_eq!(t_str.to_string(), "String");

        let t_int = DartType {
            kind: TypeKind::Int,
            is_nullable: true,
        };
        assert_eq!(t_int.to_string(), "int?");

        let t_double = DartType {
            kind: TypeKind::Double,
            is_nullable: false,
        };
        assert_eq!(t_double.to_string(), "double");

        let t_bool = DartType {
            kind: TypeKind::Bool,
            is_nullable: false,
        };
        assert_eq!(t_bool.to_string(), "bool");

        let t_dt = DartType {
            kind: TypeKind::DateTime,
            is_nullable: false,
        };
        assert_eq!(t_dt.to_string(), "DateTime");

        let t_list = DartType {
            kind: TypeKind::List(Box::new(DartType {
                kind: TypeKind::String,
                is_nullable: true,
            })),
            is_nullable: false,
        };
        assert_eq!(t_list.to_string(), "List<String?>");

        let t_map = DartType {
            kind: TypeKind::Map(
                Box::new(DartType {
                    kind: TypeKind::String,
                    is_nullable: false,
                }),
                Box::new(DartType {
                    kind: TypeKind::Int,
                    is_nullable: true,
                }),
            ),
            is_nullable: true,
        };
        assert_eq!(t_map.to_string(), "Map<String, int?>?");

        let t_custom = DartType {
            kind: TypeKind::Custom("MyClass".to_string()),
            is_nullable: false,
        };
        assert_eq!(t_custom.to_string(), "MyClass");
    }
}
