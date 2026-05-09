use flint_build::config::FlintConfig;
use flint_build::generators;
use flint_build::parser;
use std::path::Path;

#[test]
fn test_user_model() {
    let input_path = Path::new("tests/fixtures/gold/user_model.dart");

    let config = FlintConfig::load_from_file("flint.yaml").unwrap();
    let plugin = config
        .plugins
        .unwrap()
        .get("json_serializable")
        .unwrap()
        .clone();

    let classes = parser::parse_file(input_path, &plugin).unwrap();
    let generated = generators::json_serializable::emitter::generate_full_file(
        "user_model.dart",
        classes,
        &plugin.template_path,
    );

    insta::assert_snapshot!(generated);
}
