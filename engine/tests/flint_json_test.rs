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
        .generate("user_model.dart", classes, &plugin)
        .unwrap();
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
        .generate("generic_model.dart", classes, &plugin)
        .unwrap();
    let generated = output::assemble("generic_model.dart", &[("flint_json", section)]);

    insta::assert_snapshot!(generated);
}
