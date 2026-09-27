//! Parses a Dart type as written (`Map<String, m.Money>?`, `void Function(int)?`, `(int, {String name})`)
//! into a [`Type`]. Resolution (`Type::resolved`) is filled in afterwards by the model builder.

use super::{FunctionType, NamedType, RecordType, Type};

#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
    Ident(&'a str),
    Symbol(char),
}

struct Lexer<'a> {
    tokens: Vec<(Token<'a>, usize, usize)>,
    position: usize,
    text: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        let mut tokens = Vec::new();
        let mut chars = text.char_indices().peekable();
        while let Some((start, c)) = chars.next() {
            if c.is_whitespace() {
                continue;
            }
            if c.is_alphanumeric() || c == '_' || c == '$' {
                let mut end = start + c.len_utf8();
                while let Some(&(i, next)) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' || next == '$' {
                        end = i + next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push((Token::Ident(&text[start..end]), start, end));
            } else {
                tokens.push((Token::Symbol(c), start, start + c.len_utf8()));
            }
        }
        Lexer {
            tokens,
            position: 0,
            text,
        }
    }

    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.position).map(|(token, _, _)| token)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token<'a>> {
        self.tokens
            .get(self.position + offset)
            .map(|(token, _, _)| token)
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let token = self.tokens.get(self.position).map(|(t, _, _)| t.clone());
        self.position += 1;
        token
    }

    fn eat(&mut self, symbol: char) -> bool {
        if self.peek() == Some(&Token::Symbol(symbol)) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn start(&self) -> usize {
        self.tokens
            .get(self.position)
            .map_or(self.text.len(), |(_, start, _)| *start)
    }

    fn end_of_previous(&self) -> usize {
        self.position
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map_or(0, |(_, _, end)| *end)
    }

    fn source(&self, start: usize) -> String {
        let end = self.end_of_previous().max(start);
        self.text[start..end].to_string()
    }
}

/// Parses a type as written. Anything it can't make sense of becomes a named type whose `name` is the whole
/// text, so nothing is lost.
pub fn parse_type(text: &str) -> Type {
    let mut lexer = Lexer::new(text);
    match parse(&mut lexer) {
        Some(parsed) if lexer.peek().is_none() => parsed,
        _ => named(text.trim(), None, Vec::new(), false, text.trim()),
    }
}

fn named(
    name: &str,
    prefix: Option<&str>,
    arguments: Vec<Type>,
    nullable: bool,
    source: &str,
) -> Type {
    Type {
        source: source.to_string(),
        name: name.to_string(),
        prefix: prefix.map(str::to_string),
        arguments,
        nullable,
        function: None,
        record: None,
        resolved: None,
    }
}

fn parse(lexer: &mut Lexer) -> Option<Type> {
    let start = lexer.start();
    let mut current = match lexer.peek()? {
        Token::Symbol('(') => parse_record(lexer, start)?,
        Token::Ident("Function") if matches!(lexer.peek_at(1), Some(Token::Symbol('(' | '<'))) => {
            lexer.next();
            parse_function_rest(lexer, None, start)?
        }
        Token::Ident(_) => parse_named(lexer, start)?,
        Token::Symbol(_) => return None,
    };
    // `R Function(…)`, possibly repeated: `int Function() Function()`.
    while lexer.peek() == Some(&Token::Ident("Function")) {
        lexer.next();
        current = parse_function_rest(lexer, Some(current), start)?;
    }
    Some(current)
}

fn parse_named(lexer: &mut Lexer, start: usize) -> Option<Type> {
    let Some(Token::Ident(first)) = lexer.next() else {
        return None;
    };
    let (prefix, name) = if lexer.peek() == Some(&Token::Symbol('.')) {
        lexer.next();
        let Some(Token::Ident(second)) = lexer.next() else {
            return None;
        };
        (Some(first), second)
    } else {
        (None, first)
    };
    let mut arguments = Vec::new();
    if lexer.eat('<') {
        loop {
            arguments.push(parse(lexer)?);
            if lexer.eat(',') {
                continue;
            }
            if lexer.eat('>') {
                break;
            }
            return None;
        }
    }
    let nullable = lexer.eat('?');
    let source = lexer.source(start);
    Some(named(name, prefix, arguments, nullable, &source))
}

/// After `Function`: optional type parameters, the parameter list, and `?`.
fn parse_function_rest(lexer: &mut Lexer, return_type: Option<Type>, start: usize) -> Option<Type> {
    let mut type_parameters = Vec::new();
    if lexer.eat('<') {
        loop {
            let Some(Token::Ident(name)) = lexer.next() else {
                return None;
            };
            type_parameters.push(name.to_string());
            // `T extends Bound`: the bound isn't kept here.
            if lexer.peek() == Some(&Token::Ident("extends")) {
                lexer.next();
                parse(lexer)?;
            }
            if lexer.eat(',') {
                continue;
            }
            if lexer.eat('>') {
                break;
            }
            return None;
        }
    }
    if !lexer.eat('(') {
        return None;
    }
    let (positional, named_params) = parse_parameter_types(lexer, ')')?;
    let nullable = lexer.eat('?');
    Some(Type {
        source: lexer.source(start),
        name: "Function".to_string(),
        prefix: None,
        arguments: Vec::new(),
        nullable,
        function: Some(FunctionType {
            return_type: return_type.map(Box::new),
            type_parameters,
            positional,
            named: named_params,
        }),
        record: None,
        resolved: None,
    })
}

fn parse_record(lexer: &mut Lexer, start: usize) -> Option<Type> {
    lexer.next(); // `(`
    let (positional, named_fields) = parse_parameter_types(lexer, ')')?;
    let nullable = lexer.eat('?');
    Some(Type {
        source: lexer.source(start),
        name: String::new(),
        prefix: None,
        arguments: Vec::new(),
        nullable,
        function: None,
        record: Some(RecordType {
            positional,
            named: named_fields,
        }),
        resolved: None,
    })
}

/// A parameter or record field list, up to and including `close`: `int, [String? b], {required int c}`.
/// Positional names are dropped.
fn parse_parameter_types(lexer: &mut Lexer, close: char) -> Option<(Vec<Type>, Vec<NamedType>)> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    loop {
        if lexer.eat(close) {
            return Some((positional, named));
        }
        if lexer.eat('[') {
            while !lexer.eat(']') {
                positional.push(parse(lexer)?);
                skip_name(lexer);
                lexer.eat(',');
            }
        } else if lexer.eat('{') {
            while !lexer.eat('}') {
                let required = lexer.peek() == Some(&Token::Ident("required"))
                    && !matches!(lexer.peek_at(1), Some(Token::Symbol(_)));
                if required {
                    lexer.next();
                }
                let ty = parse(lexer)?;
                let Some(Token::Ident(name)) = lexer.next() else {
                    return None;
                };
                named.push(NamedType {
                    name: name.to_string(),
                    ty,
                    required,
                });
                lexer.eat(',');
            }
        } else {
            positional.push(parse(lexer)?);
            skip_name(lexer);
        }
        lexer.eat(',');
    }
}

/// Skips a parameter or field name after its type (`int count`).
fn skip_name(lexer: &mut Lexer) {
    if matches!(lexer.peek(), Some(Token::Ident(_))) {
        lexer.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compact rendering: `name<args>?`, `prefix.name`, `R Function(A, {B b})`, `(A, {B b})`.
    fn show(t: &Type) -> String {
        let q = if t.nullable { "?" } else { "" };
        if let Some(f) = &t.function {
            let ret = f
                .return_type
                .as_ref()
                .map(|r| show(r) + " ")
                .unwrap_or_default();
            let tp = if f.type_parameters.is_empty() {
                String::new()
            } else {
                format!("<{}>", f.type_parameters.join(", "))
            };
            return format!("{ret}Function{tp}({}){q}", params(&f.positional, &f.named));
        }
        if let Some(r) = &t.record {
            return format!("({}){q}", params(&r.positional, &r.named));
        }
        let prefix = t
            .prefix
            .as_ref()
            .map(|p| format!("{p}."))
            .unwrap_or_default();
        let args = if t.arguments.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                t.arguments.iter().map(show).collect::<Vec<_>>().join(", ")
            )
        };
        format!("{prefix}{}{args}{q}", t.name)
    }

    fn params(positional: &[Type], named: &[NamedType]) -> String {
        let mut parts: Vec<String> = positional.iter().map(show).collect();
        if !named.is_empty() {
            let named: Vec<String> = named
                .iter()
                .map(|n| {
                    format!(
                        "{}{} {}",
                        if n.required { "required " } else { "" },
                        show(&n.ty),
                        n.name
                    )
                })
                .collect();
            parts.push(format!("{{{}}}", named.join(", ")));
        }
        parts.join(", ")
    }

    #[test]
    fn test_parse_types() {
        for (text, expected) in [
            ("int", "int"),
            ("String?", "String?"),
            (
                "Map<String, List<m.Money?>>?",
                "Map<String, List<m.Money?>>?",
            ),
            ("m.Money", "m.Money"),
            ("void Function(int)?", "void Function(int)?"),
            ("Function", "Function"),
            ("Function(int a, [String? b])", "Function(int, String?)"),
            (
                "T Function<T>(T value, {required int count, String? label})",
                "T Function<T>(T, {required int count, String? label})",
            ),
            (
                "(int, String name, {bool flag})",
                "(int, String, {bool flag})",
            ),
            ("(int, {String name})?", "(int, {String name})?"),
            ("int Function() Function()", "int Function() Function()"),
            ("Future<List<(int, String)>>", "Future<List<(int, String)>>"),
        ] {
            assert_eq!(show(&parse_type(text)), expected, "{text}");
        }
        let money = parse_type("List<m.Money?>");
        assert_eq!(money.source, "List<m.Money?>");
        assert_eq!(money.arguments[0].source, "m.Money?");
        assert_eq!(money.arguments[0].prefix.as_deref(), Some("m"));
        // Nothing is lost on text it can't parse.
        assert_eq!(parse_type("Map<int").name, "Map<int");
    }
}
