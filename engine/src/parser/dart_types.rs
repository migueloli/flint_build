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
