use flint_build::config::FlintConfig;
use flint_build::generators;
use flint_build::output;
use flint_build::parser;
use std::path::Path;

#[test]
fn test_user_model() {
    let input_path = Path::new("tests/fixtures/gold/user_model.dart");

    let config = FlintConfig::load_from_file("tests/fixtures/flint.yaml").unwrap();
    let plugin = config.plugins.unwrap().get("flint_json").unwrap().clone();

    let classes = parser::parse_file(input_path).unwrap();
    let generator: Box<dyn generators::Generator> =
        Box::new(generators::flint_json::emitter::FlintJsonGenerator);
    let section = generator
        .generate("user_model.dart", classes, &plugin, &Default::default())
        .unwrap()
        .code;
    let generated = output::assemble("user_model.dart", &[("flint_json", section)]);

    insta::assert_snapshot!(generated);
}

#[test]
fn test_generic_model() {
    let input_path = Path::new("tests/fixtures/gold/generic_model.dart");

    let config = FlintConfig::load_from_file("tests/fixtures/flint.yaml").unwrap();
    let plugin = config.plugins.unwrap().get("flint_json").unwrap().clone();

    let classes = parser::parse_file(input_path).unwrap();
    let generator: Box<dyn generators::Generator> =
        Box::new(generators::flint_json::emitter::FlintJsonGenerator);
    let section = generator
        .generate("generic_model.dart", classes, &plugin, &Default::default())
        .unwrap()
        .code;
    let generated = output::assemble("generic_model.dart", &[("flint_json", section)]);

    insta::assert_snapshot!(generated);
}

#[test]
fn test_core_types_model() {
    let input_path = Path::new("tests/fixtures/gold/core_types_model.dart");

    let config = FlintConfig::load_from_file("tests/fixtures/flint.yaml").unwrap();
    let plugin = config.plugins.unwrap().get("flint_json").unwrap().clone();

    let classes = parser::parse_file(input_path).unwrap();
    let generator: Box<dyn generators::Generator> =
        Box::new(generators::flint_json::emitter::FlintJsonGenerator);
    let section = generator
        .generate(
            "core_types_model.dart",
            classes,
            &plugin,
            &Default::default(),
        )
        .unwrap()
        .code;
    let generated = output::assemble("core_types_model.dart", &[("flint_json", section)]);

    insta::assert_snapshot!(generated);
}

/// Cross-file resolution needs the whole package, so this one runs a real build on a copy of
/// `tests/fixtures/cross_file` (spec 0005 step 5).
#[test]
fn test_cross_file_enums() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new("tests/fixtures/cross_file");
    for relative in [
        "lib/cross_file_model.dart",
        "lib/src/status.dart",
        "lib/src/mood.dart",
        "lib/export_model.dart",
        "lib/src/catalog.dart",
        "lib/src/grade.dart",
        "lib/src/label.dart",
    ] {
        let target = dir.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(fixture.join(relative), target).unwrap();
    }
    std::fs::write(dir.path().join("flint.yaml"), "plugins:\n  flint_json:\n").unwrap();

    let mut registry = flint_build::registry::PluginRegistry::new();
    registry.register(
        "flint_json",
        Box::new(generators::flint_json::emitter::FlintJsonGenerator),
    );
    let pubspec: flint_build::config::Pubspec = "name: golden\n".parse().unwrap();
    let report = flint_build::builder::build(dir.path(), &pubspec, false, &registry).unwrap();
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    let generated =
        std::fs::read_to_string(dir.path().join("lib/cross_file_model.g.dart")).unwrap();
    insta::assert_snapshot!(generated);

    // Through a barrel file's `export`s, imported as package:<this package>/….
    let generated = std::fs::read_to_string(dir.path().join("lib/export_model.g.dart")).unwrap();
    insta::assert_snapshot!("export_model", generated);
}
