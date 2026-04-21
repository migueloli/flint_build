use crate::parser::dart_types::{DartClass, DartField, DartType};
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Scans a file for classes annotated with @JsonSerializable.
pub fn parse_file(path: &Path) -> Result<Vec<DartClass>> {
    let content = fs::read_to_string(path)?;
    let mut classes = Vec::new();

    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // 1. Look for the annotation
        if trimmed == "@JsonSerializable()" || trimmed == "@jsonSerializable" {
            // 2. The next non-empty line should be the class declaration
            while let Some(next_line) = lines.next() {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with("class ") {
                    let class_name = extract_class_name(next_trimmed);

                    // 3. Simple field extraction (MVP level)
                    let fields = extract_fields(&mut lines);

                    classes.push(DartClass {
                        name: class_name,
                        fields,
                    });
                    break;
                }
            }
        }
    }

    Ok(classes)
}

fn extract_class_name(line: &str) -> String {
    // "class User {" -> "User"
    line.split_whitespace()
        .nth(1)
        .unwrap_or("Unknown")
        .replace('{', "")
        .to_string()
}

fn extract_fields(lines: &mut std::iter::Peekable<std::str::Lines>) -> Vec<DartField> {
    let mut fields = Vec::new();
    while let Some(line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed == "}" {
            break;
        }

        if (trimmed.starts_with("final ") || trimmed.contains(" ")) && trimmed.ends_with(";") {
            let parts = trimmed.split_whitespace().collect::<Vec<&str>>();
            if let Some(field) = parse_field(parts) {
                fields.push(field);
            }
        }
        lines.next();
    }
    fields
}

fn parse_field(parts: Vec<&str>) -> Option<DartField> {
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
