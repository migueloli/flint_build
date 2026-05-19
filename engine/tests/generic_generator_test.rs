use flint_build::config::PluginConfig;
use flint_build::generators::Generator;
use flint_build::generators::generic::GenericTeraGenerator;
use flint_build::parser::dart_types::{DartClass, ParsedFile};
use std::collections::HashMap;

#[test]
fn test_generic_generator_execution() {
    let mut metadata = HashMap::new();
    metadata.insert("MockAnnotation".to_string(), "".to_string());

    let class = DartClass {
        name: "MockUser".to_string(),
        fields: vec![],
        metadata,
        type_parameters: vec![],
    };

    let parsed_file = ParsedFile {
        classes: vec![class],
        enums: vec![],
    };

    let temp_dir = std::env::temp_dir();
    let template_path = temp_dir.join("mock_template.tera");
    std::fs::write(&template_path, "Hello {{ classes[0].name }}!").unwrap();

    let config = PluginConfig {
        class_annotations: vec!["@MockAnnotation".to_string()],
        enum_annotations: vec![],
        field_annotations: vec![],
        variant_annotations: vec![],
        field_rename: None,
        converters: None,
        template_path: Some(template_path.to_str().unwrap().to_string()),
    };

    let generator = GenericTeraGenerator {
        plugin_name: "mock_plugin".to_string(),
    };
    let output = generator.generate("mock.dart", parsed_file, &config);
    assert_eq!(output, "Hello MockUser!");
}
