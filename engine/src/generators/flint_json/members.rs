//! Which members a class serializes, and how `fromJson` builds it: the constructor to call, which member
//! each parameter takes, and which members are set afterwards with cascades (spec 0006).

use crate::error::FlintError;
use crate::parser::dart_types::{
    DartClass, DartConstructor, DartField, DartGetter, DartType, Initializes, ParameterKind,
    TypeKind,
};

/// A JSON property: a field, or a getter turned into a field-like value.
#[derive(Debug, Clone)]
pub struct Member {
    pub field: DartField,
    /// Can be assigned after construction: a non-`final` field, a `late final` one without an
    /// initialiser, or a getter with a matching setter.
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Positional,
    Named(String),
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub slot: Slot,
    /// Index into [`Plan::members`]. `None` for a positional parameter no member fills, passed only to reach
    /// a later one: its value is `default`, or `null`.
    pub member: Option<usize>,
    /// The constructor parameter's default value (Dart source), if it has one.
    pub default: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FromJsonPlan {
    /// `Point` or `Point.create`.
    pub constructor: String,
    pub arguments: Vec<Argument>,
    /// Members set with `..name = value` after the constructor call.
    pub cascades: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub members: Vec<Member>,
    /// `None` when `fromJson` isn't generated (`createFactory: false`).
    pub from_json: Option<FromJsonPlan>,
    /// The members `toJson` writes, in member order.
    pub to_json: Vec<usize>,
}

fn is(field: &DartField, key: &str, value: &str) -> bool {
    field.metadata.get(key).map(String::as_str) == Some(value)
}

fn included_in_from_json(field: &DartField) -> bool {
    !is(field, "ignore", "true") && !is(field, "includeFromJson", "false")
}

fn included_in_to_json(field: &DartField) -> bool {
    !is(field, "ignore", "true") && !is(field, "includeToJson", "false")
}

/// Why a field is left out of `fromJson`, for messages.
fn exclusion(field: &DartField) -> &'static str {
    if is(field, "ignore", "true") {
        "ignore: true"
    } else {
        "includeFromJson: false"
    }
}

/// Private members only count when their `@JsonKey` includes them explicitly, as in json_serializable.
fn counts(field: &DartField) -> bool {
    !field.is_private || is(field, "includeFromJson", "true") || is(field, "includeToJson", "true")
}

fn getter_member(getter: &DartGetter, writable: bool) -> Member {
    // A getter without a declared type returns whatever its body does; take it as it is.
    let dart_type = match &getter.dart_type.kind {
        TypeKind::Unsupported(text) if text.is_empty() => DartType {
            kind: TypeKind::Dynamic,
            is_nullable: false,
        },
        _ => getter.dart_type.clone(),
    };
    Member {
        field: DartField {
            name: getter.name.clone(),
            line: getter.line,
            dart_type,
            is_final: true,
            from_json_expr: None,
            to_json_expr: None,
            metadata: getter.metadata.clone(),
            converter: None,
            is_late: false,
            has_initializer: false,
            is_private: getter.name.starts_with('_'),
        },
        writable,
    }
}

/// The class's candidate members: its fields, then getters that don't share a field's name.
fn candidates(class: &DartClass) -> Vec<Member> {
    let mut members: Vec<Member> = class
        .fields
        .iter()
        .map(|field| Member {
            writable: !field.is_final || (field.is_late && !field.has_initializer),
            field: field.clone(),
        })
        .collect();
    members.extend(
        class
            .getters
            .iter()
            .filter(|g| !class.fields.iter().any(|f| f.name == g.name))
            .map(|g| getter_member(g, class.setters.contains(&g.name))),
    );
    members.retain(|m| counts(&m.field));
    members
}

/// The constructor `fromJson` calls: `@JsonSerializable(constructor: 'name')`, or the unnamed one. A class
/// that declares no constructor has Dart's implicit unnamed one.
fn choose_constructor(class: &DartClass) -> Result<Option<&DartConstructor>, FlintError> {
    let wanted = class
        .metadata
        .get("constructor")
        .map(|raw| raw.trim_matches(|c| c == '\'' || c == '"'))
        .filter(|name| !name.is_empty());
    if wanted.is_none() && class.constructors.is_empty() {
        return Ok(None);
    }
    if let Some(found) = class
        .constructors
        .iter()
        .find(|c| c.name.as_deref() == wanted)
    {
        return Ok(Some(found));
    }
    let others: Vec<&str> = class
        .constructors
        .iter()
        .filter_map(|c| c.name.as_deref())
        .filter(|name| *name != "fromJson")
        .collect();
    let message = match wanted {
        Some(name) => format!(
            "class '{}' has no constructor named '{name}'{}",
            class.name,
            match others.as_slice() {
                [] => String::new(),
                names => format!(" (it has: {})", names.join(", ")),
            }
        ),
        None => format!(
            "class '{}' has no unnamed constructor. Add one, or pick one with @JsonSerializable(constructor: '{}')",
            class.name,
            others.first().copied().unwrap_or("name")
        ),
    };
    Err(FlintError::Constructor {
        line: class
            .constructors
            .first()
            .map_or(class.line, |constructor| constructor.line),
        message,
    })
}

/// A constructor default as Dart source, with the class's static members qualified (`defaultLimit` →
/// `Limits.defaultLimit`): the default is copied into a top-level function, where they aren't in scope.
/// String literals are left alone.
fn qualify_statics(default: &str, class: &DartClass) -> String {
    if class.static_members.is_empty() {
        return default.to_string();
    }
    let mut out = String::with_capacity(default.len());
    let mut quote: Option<char> = None;
    let mut chars = default.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' {
                if let Some((_, escaped)) = chars.next() {
                    out.push(escaped);
                }
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            out.push(c);
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let mut end = start + c.len_utf8();
            while let Some(&(i, next)) = chars.peek() {
                if next.is_alphanumeric() || next == '_' || next == '$' {
                    end = i + next.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let word = &default[start..end];
            let after_dot = out.trim_end().ends_with('.');
            if !after_dot && class.static_members.iter().any(|s| s == word) {
                out.push_str(&class.name);
                out.push('.');
            }
            out.push_str(word);
            continue;
        }
        out.push(c);
    }
    out
}

/// Works out what the class serializes and how `fromJson` builds it. `creates_factory` is false for
/// `createFactory: false`, where no constructor is needed and nothing is dropped for being unsettable.
pub fn plan(class: &DartClass, creates_factory: bool) -> Result<Plan, FlintError> {
    let candidates = candidates(class);
    if !creates_factory {
        let to_json = (0..candidates.len())
            .filter(|&i| included_in_to_json(&candidates[i].field))
            .collect();
        return Ok(Plan {
            members: candidates,
            from_json: None,
            to_json,
        });
    }

    let constructor = choose_constructor(class)?;
    let constructor_name = match constructor.and_then(|c| c.name.as_deref()) {
        Some(name) => format!("{}.{name}", class.name),
        None => class.name.clone(),
    };
    let params = constructor.map(|c| c.params.as_slice()).unwrap_or_default();
    let line = constructor.map_or(0, |c| c.line);

    let mut set_by_constructor = vec![false; candidates.len()];
    let mut arguments = Vec::new();
    // Optional positional parameters skipped so far. A later positional argument can't be passed without
    // them, so they're filled with their default (json_serializable shifts the later values instead).
    let mut skipped_positional: Vec<Argument> = Vec::new();
    for param in params {
        let skip = |skipped: &mut Vec<Argument>| {
            if param.kind == ParameterKind::OptionalPositional {
                skipped.push(Argument {
                    slot: Slot::Positional,
                    member: None,
                    default: param.default.as_deref().map(|d| qualify_statics(d, class)),
                });
            }
        };
        let found = candidates.iter().position(|m| m.field.name == param.name);
        let Some(index) = found else {
            if param.initializes == Initializes::Super {
                return Err(FlintError::Constructor {
                    line,
                    message: format!(
                        "constructor '{constructor_name}' sets '{}' through its superclass, and Flint doesn't read superclass fields yet (spec 0006 step 3). Declare '{}' in '{}', or write fromJson by hand",
                        param.name, param.name, class.name
                    ),
                });
            }
            if param.required
                && class
                    .fields
                    .iter()
                    .any(|f| f.name == param.name && f.is_private)
            {
                return Err(FlintError::Constructor {
                    line,
                    message: format!(
                        "constructor '{constructor_name}' has a required parameter '{}' for a private field, which is only serialized with @JsonKey(includeFromJson: true, includeToJson: true). Add that to the field, or make the parameter optional",
                        param.name
                    ),
                });
            }
            if param.required {
                return Err(FlintError::Constructor {
                    line,
                    message: format!(
                        "constructor '{constructor_name}' has a required parameter '{}' that doesn't match a field or getter, so fromJson can't fill it. Give it a default, make it optional, or add a field named '{}'",
                        param.name, param.name
                    ),
                });
            }
            skip(&mut skipped_positional);
            continue;
        };
        set_by_constructor[index] = true;
        let field = &candidates[index].field;
        if !included_in_from_json(field) {
            if param.required {
                return Err(FlintError::Constructor {
                    line,
                    message: format!(
                        "constructor '{constructor_name}' has a required parameter '{}', but field '{}' is excluded from fromJson ({}). Make the parameter optional, or include the field",
                        param.name,
                        field.name,
                        exclusion(field)
                    ),
                });
            }
            skip(&mut skipped_positional);
            continue;
        }
        if param.kind != ParameterKind::Named {
            arguments.append(&mut skipped_positional);
        }
        arguments.push(Argument {
            slot: match param.kind {
                ParameterKind::Named => Slot::Named(param.name.clone()),
                _ => Slot::Positional,
            },
            member: Some(index),
            default: param.default.as_deref().map(|d| qualify_statics(d, class)),
        });
    }

    // Members fromJson can't set (final fields with an initialiser or set in the initialiser list, getters)
    // aren't members at all, unless `@JsonKey(includeToJson: true)` keeps them for toJson.
    let keep: Vec<bool> = candidates
        .iter()
        .enumerate()
        .map(|(i, m)| set_by_constructor[i] || m.writable || is(&m.field, "includeToJson", "true"))
        .collect();
    let cascades = (0..candidates.len())
        .filter(|&i| {
            !set_by_constructor[i]
                && candidates[i].writable
                && included_in_from_json(&candidates[i].field)
        })
        .collect();
    let to_json = (0..candidates.len())
        .filter(|&i| keep[i] && included_in_to_json(&candidates[i].field))
        .collect();
    Ok(Plan {
        members: candidates,
        from_json: Some(FromJsonPlan {
            constructor: constructor_name,
            arguments,
            cascades,
        }),
        to_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dart_file::parse_source;
    use std::path::Path;

    fn class(code: &str) -> DartClass {
        parse_source(code, Path::new("test.dart"))
            .unwrap()
            .classes
            .remove(0)
    }

    /// `constructor(args) ..cascades | toJson keys`, with argument names for named ones.
    fn describe(plan: &Plan) -> String {
        let name = |i: usize| plan.members[i].field.name.clone();
        let argument_name = |a: &Argument| match a.member {
            Some(i) => name(i),
            None => format!("<{}>", a.default.as_deref().unwrap_or("null")),
        };
        let from = plan.from_json.as_ref().map(|f| {
            let args: Vec<String> = f
                .arguments
                .iter()
                .map(|a| {
                    let slot = match &a.slot {
                        Slot::Positional => String::new(),
                        Slot::Named(n) => format!("{n}: "),
                    };
                    let default = a
                        .default
                        .as_ref()
                        .map(|d| format!(" ?? {d}"))
                        .unwrap_or_default();
                    format!("{slot}{}{default}", argument_name(a))
                })
                .collect();
            let cascades: Vec<String> = f
                .cascades
                .iter()
                .map(|&i| format!(" ..{}", name(i)))
                .collect();
            format!(
                "{}({}){}",
                f.constructor,
                args.join(", "),
                cascades.concat()
            )
        });
        let to: Vec<String> = plan.to_json.iter().map(|&i| name(i)).collect();
        format!("{} | {}", from.unwrap_or_else(|| "-".into()), to.join(", "))
    }

    fn plan_of(code: &str) -> Result<String, String> {
        plan(&class(code), true)
            .map(|p| describe(&p))
            .map_err(|e| e.to_string())
    }

    #[test]
    fn test_json_serializable_shapes() {
        let cases = [
            (
                "class Point { static const o = 0; final int x; final int y; final int z; final List<String> tags = const []; final int doubled; late String label; String? note; int count = 0; int get sum => x + y; Point(this.x, this.y, [this.z = 0]) : doubled = x * 2; }",
                "Point(x, y, z ?? 0) ..label ..note ..count | x, y, z, label, note, count",
            ),
            (
                "class Opts { final int a; final int b; final String? c; final int d; Opts(this.a, {required this.b, this.c, this.d = 7}); }",
                "Opts(a, b: b, c: c, d: d ?? 7) | a, b, c, d",
            ),
            (
                "class Secret { final int visible; final int _secret; Secret({required this.visible, int secret = 0}) : _secret = secret; int get secret => _secret; }",
                "Secret(visible: visible, secret: secret ?? 0) | visible, secret",
            ),
            (
                "class Multi { final int a, b; Multi(this.a, this.b); }",
                "Multi(a, b) | a, b",
            ),
            (
                "@JsonSerializable(constructor: 'create') class Made { final int x; Made._(this.x); factory Made.create({required int x}) => Made._(x); }",
                "Made.create(x: x) | x",
            ),
            (
                "class Plain { final int x; final int y; Plain(int x, {int y = 3}) : x = x, y = y; }",
                "Plain(x, y: y ?? 3) | x, y",
            ),
            (
                "class PrivKey { @JsonKey(includeFromJson: true, includeToJson: true) final int _hidden; PrivKey(this._hidden); }",
                "PrivKey(_hidden) | _hidden",
            ),
            (
                "class LateFinal { final int x; late final String y; @JsonKey(includeToJson: true) final int derived; LateFinal(this.x) : derived = x + 1; }",
                "LateFinal(x) ..y | x, y, derived",
            ),
            (
                "class Getter { final int x; Getter(this.x); @JsonKey(includeToJson: true) int get twice => x * 2; }",
                "Getter(x) | x, twice",
            ),
            (
                "class Implicit { String? a; int b = 0; }",
                "Implicit() ..a ..b | a, b",
            ),
            (
                "class Ign2 { @JsonKey(includeFromJson: false) final int x; Ign2([this.x = 1]); }",
                "Ign2() | x",
            ),
            // A skipped optional positional parameter before a filled one gets its default, or null.
            (
                "class Gap { final int x; final int a; final int? b; Gap(this.x, [int s = 0, int? t, this.a = 1, int? u]) : b = null; }",
                "Gap(x, <0> ?? 0, <null>, a ?? 1) | x, a",
            ),
        ];
        for (code, expected) in cases {
            assert_eq!(plan_of(code).as_deref(), Ok(expected), "{code}");
        }
    }

    #[test]
    fn test_getter_setter_pairs_statics_in_defaults_and_untyped_getters() {
        let class = class(
            "class A { static const defaultLimit = 10; static const other = 'defaultLimit'; final int limit; final String tag; int _x = 0; int get x => _x; set x(int v) => _x = v; get untyped => 1; A({this.limit = defaultLimit * 2, this.tag = 'defaultLimit' + other}); }",
        );
        let plan = plan(&class, true).unwrap();
        assert_eq!(
            describe(&plan),
            "A(limit: limit ?? A.defaultLimit * 2, tag: tag ?? 'defaultLimit' + A.other) ..x | limit, tag, x"
        );
        let plan_without_factory = super::plan(&class, false).unwrap();
        let untyped = plan_without_factory
            .members
            .iter()
            .find(|m| m.field.name == "untyped")
            .unwrap();
        assert_eq!(untyped.field.dart_type.kind, TypeKind::Dynamic);
    }

    #[test]
    fn test_without_from_json_every_public_member_is_written() {
        let class = class(
            "class ToOnly { final int x; final List<int> tags = const []; final int _hidden = 1; int get sum => x; ToOnly(this.x); }",
        );
        assert_eq!(describe(&plan(&class, false).unwrap()), "- | x, tags, sum");
    }

    #[test]
    fn test_errors() {
        let error = |code: &str| plan_of(code).unwrap_err();
        assert_eq!(
            error("class Bad { final int x; Bad({required this.x, required int extra}); }"),
            "line 1: constructor 'Bad' has a required parameter 'extra' that doesn't match a field or getter, so fromJson can't fill it. Give it a default, make it optional, or add a field named 'extra'."
        );
        assert_eq!(
            error(
                "class Ign {\n  @JsonKey(includeFromJson: false)\n  final int x;\n  Ign(this.x);\n}"
            ),
            "line 4: constructor 'Ign' has a required parameter 'x', but field 'x' is excluded from fromJson (includeFromJson: false). Make the parameter optional, or include the field."
        );
        assert_eq!(
            error(
                "class OnlyNamed { final int x; OnlyNamed.make(this.x); factory OnlyNamed.fromJson(Map<String, dynamic> j) => OnlyNamed.make(1); }"
            ),
            "line 1: class 'OnlyNamed' has no unnamed constructor. Add one, or pick one with @JsonSerializable(constructor: 'make')."
        );
        assert_eq!(
            error(
                "@JsonSerializable(constructor: 'nope') class N { final int x; N(this.x); N.other(this.x); }"
            ),
            "line 1: class 'N' has no constructor named 'nope' (it has: other)."
        );
        assert!(
            error("class Child extends Base { final String name; Child(super.id, this.name); }")
                .contains("sets 'id' through its superclass"),
        );
        // Private fields don't match `this._x` unless included, so a required `this._x` can't be filled.
        assert!(
            error("class P { final int _x; P(this._x); }")
                .contains("required parameter '_x' for a private field, which is only serialized with @JsonKey(includeFromJson: true, includeToJson: true)")
        );
        // Without constructors, the class's own line.
        assert_eq!(
            error("\n@JsonSerializable(constructor: 'make')\nclass NoCtor { int? x; }"),
            "line 2: class 'NoCtor' has no constructor named 'make'."
        );
    }
}
