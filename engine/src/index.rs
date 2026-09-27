//! Project-wide symbol index (spec 0005): which top-level types each file declares, and which of them a
//! type name used in a file refers to, following `part`, `import` (prefixes, `show`/`hide`) and `export`.
//! Syntax only: files outside this package (`dart:`, other packages) are never resolved.

use crate::parser::dart_types::{
    DartEnum, Declaration, DeclarationKind, Directive, DirectiveKind, ParsedFile,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// What a type name used in a file refers to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedType {
    /// `class`, `enum`, `mixin`, `type_alias`, `extension_type`, or `unresolved`.
    pub kind: &'static str,
    /// The declaring file, relative to the package root (`lib/src/money.dart`); `None` if unresolved.
    pub file: Option<String>,
    pub has_from_json: bool,
    pub has_to_json: bool,
    /// The declaring file as a path, for the up-to-date check (not part of the template context).
    #[serde(skip)]
    pub path: Option<PathBuf>,
    /// For enums: the declaration with its values, so a file that uses the enum can emit its own copy of
    /// the value map (spec 0005 step 5).
    #[serde(skip)]
    pub enum_declaration: Option<DartEnum>,
    /// For unresolved names: the file imports another package, so the name may be declared there.
    #[serde(skip)]
    pub possibly_external: bool,
}

impl ResolvedType {
    /// A name the index can't find. `possibly_external` says whether the file imports another package.
    pub fn unresolved(possibly_external: bool) -> Self {
        ResolvedType {
            kind: "unresolved",
            file: None,
            has_from_json: false,
            has_to_json: false,
            path: None,
            enum_declaration: None,
            possibly_external,
        }
    }
}

/// Resolutions for the type names used by one file's generated classes, keyed by the name as written
/// (`Money`, `m.Money`). Sorted, so anything rendered from it is deterministic.
pub type ResolvedTypes = BTreeMap<String, ResolvedType>;

#[derive(Debug, Clone, PartialEq)]
pub struct Ambiguity {
    pub name: String,
    /// Declaring files, relative to the package root, sorted.
    pub files: Vec<String>,
}

#[derive(Debug, Default)]
struct FileSymbols {
    declarations: Vec<Declaration>,
    enums: Vec<DartEnum>,
    directives: Vec<Directive>,
    parts: Vec<PathBuf>,
    part_of: Option<PathBuf>,
}

pub struct SymbolIndex {
    root: PathBuf,
    package: String,
    files: BTreeMap<PathBuf, FileSymbols>,
}

impl SymbolIndex {
    /// `root` is the package root (where `pubspec.yaml` is); `package` its name, for `package:` URIs.
    pub fn new(root: &Path, package: &str) -> Self {
        SymbolIndex {
            root: normalize(root),
            package: package.to_string(),
            files: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, path: &Path, parsed: &ParsedFile) {
        let path = normalize(path);
        let symbols = FileSymbols {
            declarations: parsed.declarations.clone(),
            enums: parsed.enums.clone(),
            directives: parsed.directives.clone(),
            parts: parsed
                .part_directives
                .iter()
                .filter_map(|uri| self.resolve_uri(&path, uri))
                .collect(),
            part_of: parsed
                .part_of
                .as_deref()
                .and_then(|uri| self.resolve_uri(&path, uri)),
        };
        self.files.insert(path, symbols);
    }

    /// Resolves `name` (`Money` or `m.Money`) as used in `from`. `Ok(None)` means it isn't declared in this
    /// package, as far as `from` can see; two different visible declarations are an [`Ambiguity`].
    pub fn resolve(&self, from: &Path, name: &str) -> Result<Option<ResolvedType>, Ambiguity> {
        let from = normalize(from);
        let (prefix, simple) = match name.rsplit_once('.') {
            Some((prefix, simple)) => (Some(prefix), simple),
            None => (None, name),
        };
        let library = self.library_of(&from);

        // Declarations in the library itself shadow imports.
        if prefix.is_none() {
            let local: Vec<(PathBuf, &Declaration)> = self
                .library_members(&library)
                .into_iter()
                .flat_map(|file| self.declared_in(&file, simple))
                .collect();
            if !local.is_empty() {
                return self.unique(name, local).map(Some);
            }
        }

        let mut found = Vec::new();
        if let Some(symbols) = self.files.get(&library) {
            for import in symbols
                .directives
                .iter()
                .filter(|d| d.kind == DirectiveKind::Import && d.prefix.as_deref() == prefix)
            {
                let Some(target) = self.resolve_uri(&library, &import.uri) else {
                    continue;
                };
                found.extend(
                    self.namespace(&target, &mut BTreeSet::new())
                        .into_iter()
                        .filter(|(_, declaration)| {
                            declaration.name == simple && visible(import, &declaration.name)
                        }),
                );
            }
        }
        if found.is_empty() {
            return Ok(None);
        }
        self.unique(name, found).map(Some)
    }

    fn unique(
        &self,
        name: &str,
        mut found: Vec<(PathBuf, &Declaration)>,
    ) -> Result<ResolvedType, Ambiguity> {
        // The same declaration can be reached through several imports or exports.
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found.dedup_by(|a, b| a.0 == b.0 && a.1.name == b.1.name);
        if found.len() > 1 {
            return Err(Ambiguity {
                name: name.to_string(),
                files: found.iter().map(|(file, _)| self.relative(file)).collect(),
            });
        }
        let (file, declaration) = &found[0];
        Ok(ResolvedType {
            kind: kind_name(declaration.kind),
            file: Some(self.relative(file)),
            has_from_json: declaration.has_from_json,
            has_to_json: declaration.has_to_json,
            path: Some(file.clone()),
            enum_declaration: match declaration.kind {
                DeclarationKind::Enum => self.files.get(file).and_then(|symbols| {
                    symbols
                        .enums
                        .iter()
                        .find(|e| e.name == declaration.name)
                        .cloned()
                }),
                _ => None,
            },
            possibly_external: false,
        })
    }

    /// Whether `from`'s library imports a package other than this one (`dart:` libraries don't count).
    pub fn imports_other_packages(&self, from: &Path) -> bool {
        let own = format!("package:{}/", self.package);
        self.files
            .get(&self.library_of(&normalize(from)))
            .is_some_and(|symbols| {
                symbols.directives.iter().any(|d| {
                    d.kind == DirectiveKind::Import
                        && d.uri.starts_with("package:")
                        && !d.uri.starts_with(&own)
                })
            })
    }

    fn declared_in<'a>(&'a self, file: &Path, name: &str) -> Vec<(PathBuf, &'a Declaration)> {
        self.files
            .get(file)
            .map(|symbols| {
                symbols
                    .declarations
                    .iter()
                    .filter(|d| d.name == name)
                    .map(|d| (file.to_path_buf(), d))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What importing `file` makes visible: its library's declarations plus everything it exports.
    fn namespace<'a>(
        &'a self,
        file: &Path,
        visited: &mut BTreeSet<PathBuf>,
    ) -> Vec<(PathBuf, &'a Declaration)> {
        let library = self.library_of(file);
        if !visited.insert(library.clone()) {
            return Vec::new(); // export cycle
        }
        let mut names: Vec<(PathBuf, &Declaration)> = self
            .library_members(&library)
            .into_iter()
            .flat_map(|member| {
                self.files
                    .get(&member)
                    .into_iter()
                    .flat_map(|symbols| symbols.declarations.iter())
                    .map(move |d| (member.clone(), d))
            })
            .collect();
        if let Some(symbols) = self.files.get(&library) {
            for export in symbols
                .directives
                .iter()
                .filter(|d| d.kind == DirectiveKind::Export)
            {
                if let Some(target) = self.resolve_uri(&library, &export.uri) {
                    names.extend(
                        self.namespace(&target, visited)
                            .into_iter()
                            .filter(|(_, d)| visible(export, &d.name)),
                    );
                }
            }
        }
        names
    }

    /// The library file a file belongs to (itself, unless it's a `part of` another file).
    fn library_of(&self, file: &Path) -> PathBuf {
        self.files
            .get(file)
            .and_then(|symbols| symbols.part_of.clone())
            .unwrap_or_else(|| file.to_path_buf())
    }

    fn library_members(&self, library: &Path) -> Vec<PathBuf> {
        let mut members = vec![library.to_path_buf()];
        if let Some(symbols) = self.files.get(library) {
            members.extend(symbols.parts.iter().cloned());
        }
        members
    }

    /// A URI in `from` as a project file, or `None` for `dart:` and other packages.
    fn resolve_uri(&self, from: &Path, uri: &str) -> Option<PathBuf> {
        if uri.starts_with("dart:") {
            return None;
        }
        if let Some(rest) = uri.strip_prefix("package:") {
            let (package, path) = rest.split_once('/')?;
            return (package == self.package).then(|| normalize(&self.root.join("lib").join(path)));
        }
        if uri.contains(':') {
            return None; // other schemes
        }
        Some(normalize(&from.parent()?.join(uri)))
    }

    fn relative(&self, file: &Path) -> String {
        file.strip_prefix(&self.root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

fn visible(directive: &Directive, name: &str) -> bool {
    (directive.show.is_empty() || directive.show.iter().any(|n| n == name))
        && !directive.hide.iter().any(|n| n == name)
}

fn kind_name(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Class => "class",
        DeclarationKind::Enum => "enum",
        DeclarationKind::Mixin => "mixin",
        DeclarationKind::TypeAlias => "type_alias",
        DeclarationKind::ExtensionType => "extension_type",
    }
}

/// Lexically normalizes a path (`./lib/a/../b.dart` → `lib/b.dart`), so the same file always has one key.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use std::fs;

    /// A package named `app` whose files are given as (path under the root, content).
    fn index(files: &[(&str, &str)]) -> (tempfile::TempDir, SymbolIndex) {
        let dir = tempfile::tempdir().unwrap();
        let mut index = SymbolIndex::new(dir.path(), "app");
        for (path, content) in files {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, content).unwrap();
        }
        for (path, _) in files {
            let full = dir.path().join(path);
            index.add(&full, &parse_file(&full).unwrap());
        }
        (dir, index)
    }

    fn resolve(
        dir: &tempfile::TempDir,
        index: &SymbolIndex,
        from: &str,
        name: &str,
    ) -> Option<String> {
        index
            .resolve(&dir.path().join(from), name)
            .unwrap()
            .map(|r| format!("{} {}", r.kind, r.file.unwrap()))
    }

    #[test]
    fn test_resolves_through_the_library_imports_prefixes_and_exports() {
        let (dir, index) = index(&[
            (
                "lib/model.dart",
                "import 'src/color.dart';\nimport 'package:app/src/money.dart' as m;\nimport 'package:other/other.dart';\nimport 'api.dart';\npart 'model.part.dart';\nenum Local { a }\n",
            ),
            (
                "lib/model.part.dart",
                "part of 'model.dart';\nclass InPart {}\n",
            ),
            ("lib/src/color.dart", "enum Color { red }\n"),
            (
                "lib/src/money.dart",
                "class Money { Money.fromJson(Map<String, dynamic> j); Map<String, dynamic> toJson() => {}; }\n",
            ),
            ("lib/api.dart", "export 'src/api/user.dart';\n"),
            ("lib/src/api/user.dart", "class User {}\n"),
        ]);
        let r = |name| resolve(&dir, &index, "lib/model.dart", name);

        assert_eq!(r("Local").as_deref(), Some("enum lib/model.dart"));
        assert_eq!(r("InPart").as_deref(), Some("class lib/model.part.dart"));
        assert_eq!(r("Color").as_deref(), Some("enum lib/src/color.dart"));
        assert_eq!(r("m.Money").as_deref(), Some("class lib/src/money.dart"));
        assert_eq!(r("User").as_deref(), Some("class lib/src/api/user.dart"));
        // Only visible through the prefix; and other packages aren't indexed.
        assert_eq!(r("Money"), None);
        assert_eq!(r("Other"), None);
        // A part file sees its library's imports.
        assert_eq!(
            resolve(&dir, &index, "lib/model.part.dart", "Color").as_deref(),
            Some("enum lib/src/color.dart")
        );

        let money = index
            .resolve(&dir.path().join("lib/model.dart"), "m.Money")
            .unwrap()
            .unwrap();
        assert!(money.has_from_json && money.has_to_json);

        // model.dart (and its part) imports package:other; its own `package:app/` and `dart:` don't count.
        let path = |file: &str| dir.path().join(file);
        assert!(index.imports_other_packages(&path("lib/model.dart")));
        assert!(index.imports_other_packages(&path("lib/model.part.dart")));
        assert!(!index.imports_other_packages(&path("lib/api.dart")));
    }

    #[test]
    fn test_show_hide_shadowing_and_ambiguity() {
        let (dir, index) = index(&[
            ("lib/a.dart", "class Shared {}\nclass OnlyA {}\n"),
            ("lib/b.dart", "class Shared {}\n"),
            ("lib/both.dart", "import 'a.dart';\nimport 'b.dart';\n"),
            (
                "lib/picked.dart",
                "import 'a.dart' hide Shared;\nimport 'b.dart' show Shared;\n",
            ),
            ("lib/local.dart", "import 'a.dart';\nclass Shared {}\n"),
            ("lib/cycle_a.dart", "export 'cycle_b.dart';\nclass A {}\n"),
            ("lib/cycle_b.dart", "export 'cycle_a.dart';\n"),
            ("lib/uses_cycle.dart", "import 'cycle_b.dart';\n"),
        ]);

        let error = index
            .resolve(&dir.path().join("lib/both.dart"), "Shared")
            .unwrap_err();
        assert_eq!(
            error.files,
            vec!["lib/a.dart".to_string(), "lib/b.dart".to_string()]
        );
        assert_eq!(
            resolve(&dir, &index, "lib/both.dart", "OnlyA").as_deref(),
            Some("class lib/a.dart")
        );
        assert_eq!(
            resolve(&dir, &index, "lib/picked.dart", "Shared").as_deref(),
            Some("class lib/b.dart")
        );
        assert_eq!(
            resolve(&dir, &index, "lib/local.dart", "Shared").as_deref(),
            Some("class lib/local.dart")
        );
        assert_eq!(
            resolve(&dir, &index, "lib/uses_cycle.dart", "A").as_deref(),
            Some("class lib/cycle_a.dart")
        );
    }

    #[test]
    fn test_normalize() {
        assert_eq!(
            normalize(Path::new("./lib/a/../b.dart")),
            PathBuf::from("lib/b.dart")
        );
        assert_eq!(
            normalize(Path::new("/p/./lib/x.dart")),
            PathBuf::from("/p/lib/x.dart")
        );
    }
}
