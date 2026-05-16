use tree_sitter::Parser;

#[test]
fn test_parse() {
    let code = r#"
        @JsonSerializable()
        class ApiResponse<T, U> {
            final T data;
            @MyConverter()
            final DateTime date;
        }
    "#;
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_dart::LANGUAGE.into()).unwrap();
    let tree = parser.parse(code, None).unwrap();
    println!("{}", tree.root_node().to_sexp());
}
