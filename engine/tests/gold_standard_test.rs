use flint_build::generators;
use flint_build::parser;
use std::path::Path;

#[test]
fn test_gold_standard_user_model() {
    let input_path = Path::new("tests/fixtures/gold/user_model.dart");

    let classes = parser::parse_file(input_path).unwrap();
    let generated =
        generators::json_serializable::emitter::generate_full_file("user_model.dart", &classes);

    insta::assert_snapshot!(generated);
}
