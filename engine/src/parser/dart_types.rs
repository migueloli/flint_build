use serde::Serialize;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Serialize)]
pub struct ParsedFile {
    pub classes: Vec<DartClass>,
    pub enums: Vec<DartEnum>,
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
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DartType {
    pub kind: TypeKind,
    pub is_nullable: bool,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct DartField {
    pub name: String,
    pub dart_type: DartType,
    pub is_final: bool,
    pub from_json_expr: Option<String>,
    pub to_json_expr: Option<String>,
    pub metadata: HashMap<String, String>,
    pub converter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DartClass {
    pub name: String,
    pub fields: Vec<DartField>,
    pub metadata: HashMap<String, String>,
    pub type_parameters: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DartEnumValue {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Serialize)]
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
            TypeKind::Custom(name) => write!(f, "{}", name),
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
