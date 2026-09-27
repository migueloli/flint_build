//! The generator model, version 1 (spec 0007): what every generator receives, whatever language it's
//! written in. Built from a file's syntax tree and the project symbol index.
//!
//! The JSON form is the contract with Dart generators and templates, and is published as a JSON Schema
//! (`docs/model/v1.schema.json`). Additive changes (new optional fields) keep [`MODEL_VERSION`]'s major
//! number; anything else bumps it.

mod build;
pub mod types;

pub use build::library;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `major.minor`. Generators declare the major version they support.
pub const MODEL_VERSION: &str = "1.0";

/// One Dart file. A part file has its own entry with `part_of` set; its declarations belong to that
/// library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Library {
    /// `package:app/models/user.dart` for files under `lib/`; otherwise the path (`test/user_test.dart`).
    pub uri: String,
    /// Relative to the package root.
    pub path: String,
    pub imports: Vec<Directive>,
    pub exports: Vec<Directive>,
    /// URIs of `part '…';` directives, as written.
    pub parts: Vec<String>,
    /// The URI of `part of '…';`, as written.
    pub part_of: Option<String>,
    pub classes: Vec<Class>,
    pub enums: Vec<Enum>,
    pub mixins: Vec<Mixin>,
    pub extensions: Vec<Extension>,
    pub extension_types: Vec<ExtensionType>,
    pub typedefs: Vec<Typedef>,
    pub functions: Vec<Function>,
    /// Top-level getters (`int get answer => 42;`).
    pub getters: Vec<Getter>,
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Directive {
    pub uri: String,
    pub prefix: Option<String>,
    pub show: Vec<String>,
    pub hide: Vec<String>,
}

/// An annotation with its arguments kept apart: `@JsonKey(name: 'id')` →
/// `{ name: "JsonKey", arguments: { named: { name: { source: "'id'", literal: "id" } } } }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    /// Without `@` or prefix.
    pub name: String,
    /// `json` in `@json.JsonSerializable()`.
    pub prefix: Option<String>,
    /// `named` in `@Foo.named()`, a named constructor. `@a.B` is read as a prefix and a name when `a` is
    /// lower case (Dart's convention), and as a name and a constructor otherwise.
    pub constructor: Option<String>,
    /// `None` for an annotation without parentheses (`@immutable`).
    pub arguments: Option<Arguments>,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Arguments {
    pub positional: Vec<Expr>,
    /// Sorted by name, so the output is deterministic.
    pub named: BTreeMap<String, Expr>,
}

/// An expression as written, plus its value when it's a literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Expr {
    pub source: String,
    pub literal: Option<Literal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Literal {
    String(String),
    Int(i64),
    Double(f64),
    Bool(bool),
    Null,
    List(Vec<Literal>),
    Map(Vec<MapEntry>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEntry {
    pub key: Literal,
    pub value: Literal,
}

/// A type as written, taken apart: `Map<String, m.Money>?` →
/// `{ name: "Map", arguments: [String, { name: "Money", prefix: "m" }], nullable: true }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Type {
    pub source: String,
    /// The type's name without prefix or arguments; `Function` for function types, empty for records.
    pub name: String,
    pub prefix: Option<String>,
    pub arguments: Vec<Type>,
    pub nullable: bool,
    pub function: Option<FunctionType>,
    pub record: Option<RecordType>,
    /// What the project index says the name refers to. `None` for records and function types.
    pub resolved: Option<Resolved>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FunctionType {
    pub return_type: Option<Box<Type>>,
    pub type_parameters: Vec<String>,
    /// Positional parameters, required and optional (`[…]`) alike.
    pub positional: Vec<Type>,
    pub named: Vec<NamedType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordType {
    pub positional: Vec<Type>,
    pub named: Vec<NamedType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NamedType {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Resolved {
    pub kind: ResolvedKind,
    /// The declaring library's URI, for declarations in this package.
    pub library: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedKind {
    Class,
    Enum,
    Mixin,
    Typedef,
    ExtensionType,
    /// A type parameter of the enclosing declaration.
    TypeParameter,
    /// A `dart:core` type (`int`, `List`, `Future`, …).
    DartCore,
    /// Not declared in this package, as far as the file can see (another package, or a missing import).
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TypeParameter {
    pub name: String,
    pub bound: Option<Type>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Members {
    pub fields: Vec<Field>,
    pub getters: Vec<Getter>,
    pub setters: Vec<Setter>,
    pub methods: Vec<Method>,
    pub constructors: Vec<Constructor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Class {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    /// `abstract`, `sealed`, `final`, `base`, `interface`, `mixin`, in source order.
    pub modifiers: Vec<String>,
    pub type_parameters: Vec<TypeParameter>,
    pub superclass: Option<Type>,
    pub mixins: Vec<Type>,
    pub interfaces: Vec<Type>,
    #[serde(flatten)]
    pub members: Members,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Enum {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub type_parameters: Vec<TypeParameter>,
    pub mixins: Vec<Type>,
    pub interfaces: Vec<Type>,
    pub values: Vec<EnumValue>,
    #[serde(flatten)]
    pub members: Members,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnumValue {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    /// Constructor arguments of an enhanced enum's value (`red('R', 1)`).
    pub arguments: Option<Arguments>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mixin {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    /// `base` for `base mixin`.
    pub modifiers: Vec<String>,
    pub type_parameters: Vec<TypeParameter>,
    /// The `on` clause.
    pub on: Vec<Type>,
    pub interfaces: Vec<Type>,
    #[serde(flatten)]
    pub members: Members,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Extension {
    /// `None` for an unnamed extension.
    pub name: Option<String>,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub type_parameters: Vec<TypeParameter>,
    pub on: Option<Type>,
    #[serde(flatten)]
    pub members: Members,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionType {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub type_parameters: Vec<TypeParameter>,
    /// The representation field: `value` and `int` in `extension type Id(int value)`.
    pub representation_name: String,
    pub representation_type: Option<Type>,
    pub interfaces: Vec<Type>,
    #[serde(flatten)]
    pub members: Members,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Typedef {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub type_parameters: Vec<TypeParameter>,
    /// The aliased type. For the old form `typedef void F(int x);` it's the equivalent function type.
    pub aliased: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Field {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    /// `None` for `var x;` or `final x = 1;`.
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    pub is_static: bool,
    pub is_final: bool,
    pub is_const: bool,
    pub is_late: bool,
    pub initializer: Option<Expr>,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Getter {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    pub is_static: bool,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Setter {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    /// The parameter's type.
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    pub is_static: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Method {
    /// For operators, the operator (`+`, `==`, `[]`).
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub return_type: Option<Type>,
    pub type_parameters: Vec<TypeParameter>,
    pub params: Vec<Parameter>,
    pub is_static: bool,
    /// Declared without a body.
    pub is_abstract: bool,
    pub is_operator: bool,
    /// The body's modifier: `async`, `async*` or `sync*`.
    pub body_modifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Function {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub return_type: Option<Type>,
    pub type_parameters: Vec<TypeParameter>,
    pub params: Vec<Parameter>,
    /// The body's modifier: `async`, `async*` or `sync*`.
    pub body_modifier: Option<String>,
    pub is_external: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Variable {
    pub name: String,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    pub is_final: bool,
    pub is_const: bool,
    pub is_late: bool,
    pub initializer: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConstructorKind {
    Generative,
    Factory,
    /// `factory X(…) = Y;`, as freezed uses.
    RedirectingFactory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Constructor {
    /// `None` for the unnamed constructor.
    pub name: Option<String>,
    pub line: usize,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub kind: ConstructorKind,
    pub is_const: bool,
    /// The target of a redirecting factory (`_Circle<T>`).
    pub redirects_to: Option<Type>,
    pub params: Vec<Parameter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    Positional,
    OptionalPositional,
    Named,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Initializes {
    /// `this.x`
    This,
    /// `super.x`
    Super,
    /// `int x`
    Plain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    /// Positional parameters, and named ones marked `required`.
    pub required: bool,
    pub default: Option<Expr>,
    pub initializes: Initializes,
    /// The declared type; `None` for `this.x`/`super.x` without one (the field's type applies).
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    /// Annotations on the parameter (`@Default(0)`, `@JsonKey(…)`).
    pub annotations: Vec<Annotation>,
}

/// `dump-model` output: the model version and one entry per requested file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelDump {
    pub model_version: String,
    pub libraries: Vec<Library>,
}

/// The JSON Schema of [`ModelDump`], as published in `docs/model/v1.schema.json`.
pub fn schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ModelDump)).unwrap_or_default()
}
