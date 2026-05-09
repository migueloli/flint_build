use serde::Serialize;
use std::{collections::HashMap, fmt::Display};

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
}

#[derive(Debug, Serialize)]
pub struct DartClass {
    pub name: String,
    pub fields: Vec<DartField>,
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
