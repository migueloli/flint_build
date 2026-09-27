//! The generator model (spec 0007): what `dump-model` prints and generators receive.

use flint_build::builder::dump_model;
use flint_build::config::Pubspec;
use flint_build::model::{self, Library, ModelDump, ResolvedKind};
use std::path::{Path, PathBuf};

/// The package name from `<root>/pubspec.yaml`, so `package:` imports resolve as in a real build.
fn package_name(root: &str) -> String {
    let pubspec: Pubspec = std::fs::read_to_string(Path::new(root).join("pubspec.yaml"))
        .unwrap()
        .parse()
        .unwrap();
    pubspec.name
}

fn dump(root: &str, files: &[&str]) -> ModelDump {
    let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    let (dump, errors) = dump_model(Path::new(root), &package_name(root), &files).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    dump
}

#[test]
fn test_model_fixture() {
    let dump = dump("tests/fixtures/model", &["lib/model_fixture.dart"]);
    insta::assert_snapshot!(serde_json::to_string_pretty(&dump).unwrap());
}

#[test]
fn test_every_golden_fixture_validates_against_the_schema_and_round_trips() {
    let schema = model::schema();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for root in ["tests/dart_golden", "tests/fixtures/model"] {
        let dump = dump(root, &[]);
        assert!(!dump.libraries.is_empty(), "{root}");
        let json = serde_json::to_value(&dump).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&json)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{root}: {errors:#?}");
        let back: ModelDump = serde_json::from_value(json).unwrap();
        assert_eq!(back, dump, "{root}");
    }
}

#[test]
fn test_published_schema_is_current() {
    let published = std::fs::read_to_string("../docs/model/v1.schema.json").unwrap_or_default();
    let current = serde_json::to_string_pretty(&model::schema()).unwrap() + "\n";
    assert!(
        published == current,
        "docs/model/v1.schema.json is stale. Regenerate it from engine/ with:\n  cargo run -q -- dump-model --schema > ../docs/model/v1.schema.json"
    );
}

fn library<'a>(dump: &'a ModelDump, path: &str) -> &'a Library {
    dump.libraries.iter().find(|l| l.path == path).unwrap()
}

#[test]
fn test_types_resolve_across_the_golden_package() {
    // `export_model.dart` reaches `Grade` and `Label` through `package:flint_dart_golden/src/catalog.dart`.
    let dump = dump("tests/dart_golden", &["lib/export_model.dart"]);
    let shelf = &library(&dump, "lib/export_model.dart").classes[0];
    let resolved = |name: &str| {
        let field = shelf
            .members
            .fields
            .iter()
            .find(|f| f.name == name)
            .unwrap();
        field.ty.as_ref().unwrap().resolved.clone().unwrap()
    };
    assert_eq!(resolved("level").kind, ResolvedKind::Enum);
    assert_eq!(
        resolved("level").library.as_deref(),
        Some("package:flint_dart_golden/src/grade.dart")
    );
    assert_eq!(resolved("tag").kind, ResolvedKind::Class);
}

#[test]
fn test_files_outside_lib_and_absolute_paths() {
    let root = "tests/fixtures/model";
    let test_file = dump(root, &["test/fixture_test.dart"]);
    let holder = &library(&test_file, "test/fixture_test.dart");
    assert_eq!(holder.uri, "test/fixture_test.dart");
    let money = holder.classes[0].members.fields[0].ty.as_ref().unwrap();
    assert_eq!(
        money.resolved.as_ref().unwrap().library.as_deref(),
        Some("package:fixture/src/money.dart")
    );

    let absolute = std::fs::canonicalize("tests/fixtures/model/lib/model_fixture.dart").unwrap();
    let by_absolute = dump(root, &[absolute.to_str().unwrap()]);
    assert_eq!(
        by_absolute.libraries[0].uri,
        "package:fixture/model_fixture.dart"
    );
    assert_eq!(by_absolute, dump(root, &["lib/model_fixture.dart"]));
}

#[test]
fn test_a_broken_file_is_reported_and_the_rest_still_described() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lib")).unwrap();
    std::fs::write(dir.path().join("lib/ok.dart"), "class Ok {}\n").unwrap();
    std::fs::write(dir.path().join("lib/broken.dart"), "class Broken {\n").unwrap();

    let (dump, errors) = dump_model(dir.path(), "app", &[]).unwrap();

    assert_eq!(dump.libraries.len(), 1);
    assert_eq!(dump.libraries[0].classes[0].name, "Ok");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("broken.dart"), "{errors:?}");
}
