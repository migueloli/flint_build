//! The `.g.dart` files Flint owns: how they are assembled and how Flint recognises its own output
//! (spec 0001). Flint only overwrites or deletes files that carry its ownership marker.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub const HEADER: &str = "// GENERATED CODE - DO NOT MODIFY BY HAND";
/// The second line of every file Flint writes, followed by the engine version.
pub const OWNERSHIP_MARKER: &str = "// flint_build ";
/// Banner line written by `flint_json` before the ownership marker existed.
const LEGACY_MARKER: &str = "(Powered by Flint)";
/// The marker (or the legacy banner) always sits within the first few lines.
const OWNERSHIP_PREFIX_BYTES: u64 = 1024;
const BANNER_RULE: &str =
    "// **************************************************************************";

/// `lib/a/b.dart` → `lib/a/b.g.dart`.
pub fn output_path(source: &Path) -> PathBuf {
    source.with_extension("g.dart")
}

/// `lib/a/b.g.dart` → `lib/a/b.dart`, or `None` if the path isn't a `.g.dart` file.
pub fn source_path(output: &Path) -> Option<PathBuf> {
    let stem = output.file_stem()?.to_str()?.strip_suffix(".g")?;
    Some(output.with_file_name(format!("{stem}.dart")))
}

/// Whether the file was written by Flint, judged by its first bytes.
pub fn is_owned(path: &Path) -> io::Result<bool> {
    let mut prefix = Vec::new();
    File::open(path)?
        .take(OWNERSHIP_PREFIX_BYTES)
        .read_to_end(&mut prefix)?;
    Ok(String::from_utf8_lossy(&prefix)
        .lines()
        .any(|line| line.starts_with(OWNERSHIP_MARKER) || line.contains(LEGACY_MARKER)))
}

/// Builds the full `.g.dart` content: header, `part of`, then each plugin's section in the order given.
pub fn assemble(filename: &str, sections: &[(&str, String)]) -> String {
    let mut content = format!(
        "{HEADER}\n{OWNERSHIP_MARKER}{}\n\npart of '{filename}';\n",
        env!("CARGO_PKG_VERSION")
    );
    for (plugin, body) in sections {
        content.push_str(&format!(
            "\n{BANNER_RULE}\n// {plugin}\n{BANNER_RULE}\n\n{}\n",
            body.trim_end()
        ));
    }
    content
}

/// Removes the `// GENERATED CODE` header and `part of` line from the start of a section. Templates written
/// before spec 0001 emit both, and the engine now adds them itself. Only the leading block of blank and
/// comment lines is scanned; other comments in it (such as `ignore_for_file`) are kept.
pub fn strip_legacy_preamble(section: &str) -> String {
    let mut lines = section.lines().peekable();
    let mut kept = Vec::new();
    while let Some(line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(HEADER) || trimmed.starts_with("part of ") {
            lines.next();
        } else if trimmed.starts_with("//") {
            kept.push(*line);
            lines.next();
        } else {
            break;
        }
    }
    let mut result = kept.join("\n");
    let rest: Vec<&str> = lines.collect();
    if !rest.is_empty() {
        if !result.is_empty() {
            result.push_str("\n\n");
        }
        result.push_str(&rest.join("\n"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_and_source_paths() {
        assert_eq!(
            output_path(Path::new("lib/a/b.dart")),
            PathBuf::from("lib/a/b.g.dart")
        );
        assert_eq!(
            source_path(Path::new("lib/a/b.g.dart")),
            Some(PathBuf::from("lib/a/b.dart"))
        );
        assert_eq!(source_path(Path::new("lib/a/b.dart")), None);
    }

    #[test]
    fn test_assemble_orders_sections_and_marks_ownership() {
        let content = assemble(
            "user.dart",
            &[
                ("flint_json", "A\n\n".to_string()),
                ("describe", "B".to_string()),
            ],
        );
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], HEADER);
        assert!(lines[1].starts_with(OWNERSHIP_MARKER));
        assert_eq!(lines[3], "part of 'user.dart';");
        let json = content.find("// flint_json").unwrap();
        let describe = content.find("// describe").unwrap();
        assert!(json < describe);
        assert!(content.ends_with("B\n"));
    }

    #[test]
    fn test_is_owned() {
        let dir = tempfile::tempdir().unwrap();
        let owned = dir.path().join("a.g.dart");
        std::fs::write(&owned, assemble("a.dart", &[("x", "body".to_string())])).unwrap();
        assert!(is_owned(&owned).unwrap());

        let legacy = dir.path().join("b.g.dart");
        std::fs::write(
            &legacy,
            format!(
                "{HEADER}\n\npart of 'b.dart';\n\n// JsonSerializableGenerator {LEGACY_MARKER}\n"
            ),
        )
        .unwrap();
        assert!(is_owned(&legacy).unwrap());

        let foreign = dir.path().join("c.g.dart");
        std::fs::write(&foreign, format!("{HEADER}\n\npart of 'c.dart';\n")).unwrap();
        assert!(!is_owned(&foreign).unwrap());
    }

    #[test]
    fn test_strip_legacy_preamble() {
        let legacy =
            "// GENERATED CODE - DO NOT MODIFY BY HAND\n\npart of 'a.dart';\n\n  code();\n";
        assert_eq!(strip_legacy_preamble(legacy), "  code();");
        assert_eq!(strip_legacy_preamble("code();\n"), "code();");
        assert_eq!(strip_legacy_preamble("\n\n"), "");

        // The pre-0001 built-in flint_json template put `ignore_for_file` before `part of`.
        let old_builtin = "// GENERATED CODE - DO NOT MODIFY BY HAND\n// ignore_for_file: unnecessary_cast\n\npart of 'a.dart';\n\nA _$AFromJson() => A();\n";
        assert_eq!(
            strip_legacy_preamble(old_builtin),
            "// ignore_for_file: unnecessary_cast\n\nA _$AFromJson() => A();"
        );
    }
}
