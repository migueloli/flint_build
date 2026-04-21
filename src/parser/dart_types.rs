use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum DartType {
    String,
    Int,
    Double,
    Bool,
    DateTime,
    List(Box<DartType>),
    Map(Box<DartType>, Box<DartType>),
    Custom(String),
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct DartField {
    pub name: String,
    pub dart_type: DartType,
    pub is_nullable: bool,
    pub is_final: bool,
}

#[derive(Debug)]
pub struct DartClass {
    pub name: String,
    pub fields: Vec<DartField>,
}

impl Display for DartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DartType::String => write!(f, "String"),
            DartType::Int => write!(f, "int"),
            DartType::Double => write!(f, "double"),
            DartType::Bool => write!(f, "bool"),
            DartType::DateTime => write!(f, "DateTime"),
            DartType::List(inner) => write!(f, "List<{}>", inner),
            DartType::Map(key, value) => write!(f, "Map<{}, {}>", key, value),
            DartType::Custom(name) => write!(f, "{}", name),
        }
    }
}
