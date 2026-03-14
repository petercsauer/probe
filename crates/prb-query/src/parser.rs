use nom::Parser;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, multispace0},
    combinator::{map, opt, recognize},
    multi::{separated_list0, separated_list1},
    sequence::{delimited, preceded},
};

use crate::ast::{CmpOp, Expr, FieldPath, Value};
use crate::error::QueryError;

fn ws<'a, F, O>(mut inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>,
{
    move |input: &'a str| {
        let (input, _) = multispace0.parse(input)?;
        let (input, val) = inner.parse(input)?;
        let (input, _) = multispace0.parse(input)?;
        Ok((input, val))
    }
}

const fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

const fn is_ident_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn ident(input: &str) -> IResult<&str, &str> {
    recognize((
        take_while1(|c: char| is_ident_start(c)),
        take_while(|c: char| is_ident_cont(c)),
    ))
    .parse(input)
}

fn field_path(input: &str) -> IResult<&str, FieldPath> {
    map(separated_list1(char('.'), ident), |parts| {
        FieldPath(parts.into_iter().map(String::from).collect())
    })
    .parse(input)
}

fn string_lit(input: &str) -> IResult<&str, String> {
    let (input, _) = char('"').parse(input)?;
    let mut result = String::new();
    let mut chars = input.chars();
    let mut rest_start = 0;
    while let Some(c) = chars.next() {
        rest_start += c.len_utf8();
        if c == '\\' {
            if let Some(escaped) = chars.next() {
                rest_start += escaped.len_utf8();
                match escaped {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(escaped);
                    }
                }
            }
        } else if c == '"' {
            return Ok((&input[rest_start..], result));
        } else {
            result.push(c);
        }
    }
    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Char,
    )))
}

fn number_lit(input: &str) -> IResult<&str, f64> {
    let (input, neg) = opt(char('-')).parse(input)?;
    let (input, int_part) = take_while1(|c: char| c.is_ascii_digit()).parse(input)?;
    let (input, frac) = opt(preceded(
        char('.'),
        take_while1(|c: char| c.is_ascii_digit()),
    ))
    .parse(input)?;

    let mut s = String::new();
    if neg.is_some() {
        s.push('-');
    }
    s.push_str(int_part);
    if let Some(f) = frac {
        s.push('.');
        s.push_str(f);
    }
    match s.parse::<f64>() {
        Ok(n) => Ok((input, n)),
        Err(_) => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Float,
        ))),
    }
}

fn bool_lit(input: &str) -> IResult<&str, bool> {
    alt((map(tag("true"), |_| true), map(tag("false"), |_| false))).parse(input)
}

fn value(input: &str) -> IResult<&str, Value> {
    alt((
        map(string_lit, Value::String),
        map(bool_lit, Value::Bool),
        map(number_lit, Value::Number),
    ))
    .parse(input)
}

fn cmp_op(input: &str) -> IResult<&str, CmpOp> {
    alt((
        map(tag("=="), |_| CmpOp::Eq),
        map(tag("!="), |_| CmpOp::Ne),
        map(tag(">="), |_| CmpOp::Ge),
        map(tag("<="), |_| CmpOp::Le),
        map(tag(">"), |_| CmpOp::Gt),
        map(tag("<"), |_| CmpOp::Lt),
    ))
    .parse(input)
}

fn paren_expr(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, expr) = or_expr(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char(')').parse(input)?;
    Ok((input, expr))
}

fn not_expr(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char('!').parse(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, inner) = atom(input)?;
    Ok((input, Expr::Not(Box::new(inner))))
}

fn function_arg(input: &str) -> IResult<&str, Box<Expr>> {
    // Try to parse as a nested function call first
    if let Ok((rest, expr)) = function_call(input) {
        return Ok((rest, Box::new(expr)));
    }
    // Otherwise parse as field path (wrapped in Exists for evaluation)
    let (input, field) = ws(field_path).parse(input)?;
    Ok((input, Box::new(Expr::Exists { field })))
}

fn function_call(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0.parse(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, args) =
        separated_list0(delimited(multispace0, char(','), multispace0), function_arg)
            .parse(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char(')').parse(input)?;
    Ok((
        input,
        Expr::Function {
            name: name.to_string(),
            args,
        },
    ))
}

fn atom(input: &str) -> IResult<&str, Expr> {
    if let Ok(result) = paren_expr(input) {
        return Ok(result);
    }
    if let Ok(result) = not_expr(input) {
        return Ok(result);
    }
    if let Ok(result) = function_call(input) {
        return Ok(result);
    }
    field_expr(input)
}

fn field_expr(input: &str) -> IResult<&str, Expr> {
    let (input, field) = ws(field_path).parse(input)?;

    // Try slice `[start:end]`
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('[').parse(input) {
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, start) = nom::character::complete::u32(rest)?;
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, _) = char(':').parse(rest)?;
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, end) = nom::character::complete::u32(rest)?;
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, _) = char(']').parse(rest)?;
        return Ok((
            rest,
            Expr::Slice {
                field,
                start: start as usize,
                end: end as usize,
            },
        ));
    }

    // Try `matches "pattern"`
    if let Ok((rest, _)) = tag::<&str, &str, nom::error::Error<&str>>("matches").parse(input)
        && let Ok((rest, pattern)) = ws(string_lit).parse(rest)
    {
        return Ok((rest, Expr::Matches { field, pattern }));
    }

    // Try `in {val1, val2, ...}`
    if let Ok((rest, _)) = tag::<&str, &str, nom::error::Error<&str>>("in").parse(input) {
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, _) = char('{').parse(rest)?;
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, values) =
            separated_list1(delimited(multispace0, char(','), multispace0), value).parse(rest)?;
        let (rest, _) = multispace0.parse(rest)?;
        let (rest, _) = char('}').parse(rest)?;
        return Ok((rest, Expr::In { field, values }));
    }

    // Try `contains "string"`
    if let Ok((rest, _)) = tag::<&str, &str, nom::error::Error<&str>>("contains").parse(input)
        && let Ok((rest, sub)) = ws(string_lit).parse(rest)
    {
        return Ok((
            rest,
            Expr::Contains {
                field,
                substring: sub,
            },
        ));
    }

    // Try `exists`
    if let Ok((rest, _)) = tag::<&str, &str, nom::error::Error<&str>>("exists").parse(input)
        && (rest.is_empty() || !is_ident_cont(rest.chars().next().unwrap_or(' ')))
    {
        return Ok((rest, Expr::Exists { field }));
    }

    // Comparison
    let (input, op) = ws(cmp_op).parse(input)?;
    let (input, val) = ws(value).parse(input)?;
    Ok((
        input,
        Expr::Compare {
            field,
            op,
            value: val,
        },
    ))
}

fn and_expr(input: &str) -> IResult<&str, Expr> {
    let (input, first) = atom(input)?;
    let mut result = first;
    let mut remaining = input;

    loop {
        let trimmed = remaining.trim_start();
        if let Some(rest) = trimmed.strip_prefix("&&") {
            match atom(rest) {
                Ok((next_remaining, next_expr)) => {
                    result = Expr::And(Box::new(result), Box::new(next_expr));
                    remaining = next_remaining;
                }
                Err(e) => return Err(e),
            }
        } else {
            break;
        }
    }

    Ok((remaining, result))
}

fn or_expr(input: &str) -> IResult<&str, Expr> {
    let (input, first) = and_expr(input)?;
    let mut result = first;
    let mut remaining = input;

    loop {
        let trimmed = remaining.trim_start();
        if let Some(rest) = trimmed.strip_prefix("||") {
            match and_expr(rest) {
                Ok((next_remaining, next_expr)) => {
                    result = Expr::Or(Box::new(result), Box::new(next_expr));
                    remaining = next_remaining;
                }
                Err(e) => return Err(e),
            }
        } else {
            break;
        }
    }

    Ok((remaining, result))
}

pub fn parse_expr(input: &str) -> Result<Expr, QueryError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(QueryError::EmptyExpression);
    }
    match or_expr(trimmed) {
        Ok(("", expr)) => Ok(expr),
        Ok((remaining, _)) => Err(QueryError::TrailingInput(remaining.to_string())),
        Err(e) => Err(QueryError::ParseError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_eq() {
        let expr = parse_expr(r#"transport == "gRPC""#).unwrap();
        assert_eq!(
            expr,
            Expr::Compare {
                field: FieldPath(vec!["transport".into()]),
                op: CmpOp::Eq,
                value: Value::String("gRPC".into()),
            }
        );
    }

    #[test]
    fn parse_dotted_field() {
        let expr = parse_expr(r#"grpc.method == "/api/Users""#).unwrap();
        assert_eq!(
            expr,
            Expr::Compare {
                field: FieldPath(vec!["grpc".into(), "method".into()]),
                op: CmpOp::Eq,
                value: Value::String("/api/Users".into()),
            }
        );
    }

    #[test]
    fn parse_contains() {
        let expr = parse_expr(r#"grpc.method contains "Users""#).unwrap();
        assert_eq!(
            expr,
            Expr::Contains {
                field: FieldPath(vec!["grpc".into(), "method".into()]),
                substring: "Users".into(),
            }
        );
    }

    #[test]
    fn parse_exists() {
        let expr = parse_expr("warnings exists").unwrap();
        assert_eq!(
            expr,
            Expr::Exists {
                field: FieldPath(vec!["warnings".into()]),
            }
        );
    }

    #[test]
    fn parse_not() {
        let expr = parse_expr(r#"!transport == "gRPC""#).unwrap();
        assert!(matches!(expr, Expr::Not(_)));
    }

    #[test]
    fn parse_and_or() {
        let expr = parse_expr(r#"transport == "gRPC" && direction == "inbound""#).unwrap();
        assert!(matches!(expr, Expr::And(_, _)));

        let expr = parse_expr(r#"transport == "gRPC" || transport == "ZMQ""#).unwrap();
        assert!(matches!(expr, Expr::Or(_, _)));
    }

    #[test]
    fn parse_parens() {
        let expr =
            parse_expr(r#"(transport == "gRPC" || transport == "ZMQ") && direction == "inbound""#)
                .unwrap();
        assert!(matches!(expr, Expr::And(_, _)));
    }

    #[test]
    fn parse_number_comparison() {
        let expr = parse_expr("id > 42").unwrap();
        assert_eq!(
            expr,
            Expr::Compare {
                field: FieldPath(vec!["id".into()]),
                op: CmpOp::Gt,
                value: Value::Number(42.0),
            }
        );
    }

    #[test]
    fn parse_bool_value() {
        let expr = parse_expr("warnings == true").unwrap();
        assert_eq!(
            expr,
            Expr::Compare {
                field: FieldPath(vec!["warnings".into()]),
                op: CmpOp::Eq,
                value: Value::Bool(true),
            }
        );
    }

    #[test]
    fn parse_empty_is_error() {
        assert!(parse_expr("").is_err());
        assert!(parse_expr("   ").is_err());
    }

    #[test]
    fn parse_trailing_input_is_error() {
        assert!(parse_expr(r#"transport == "gRPC" garbage"#).is_err());
    }

    #[test]
    fn parse_string_escaping() {
        let expr = parse_expr(r#"msg == "line1\nline2""#).unwrap();
        if let Expr::Compare {
            value: Value::String(s),
            ..
        } = expr
        {
            assert_eq!(s, "line1\nline2");
        } else {
            panic!("Expected string with newline");
        }

        let expr = parse_expr(r#"msg == "tab\there""#).unwrap();
        if let Expr::Compare {
            value: Value::String(s),
            ..
        } = expr
        {
            assert_eq!(s, "tab\there");
        } else {
            panic!("Expected string with tab");
        }

        let expr = parse_expr(r#"msg == "backslash\\quote\"end""#).unwrap();
        if let Expr::Compare {
            value: Value::String(s),
            ..
        } = expr
        {
            assert_eq!(s, "backslash\\quote\"end");
        } else {
            panic!("Expected string with escaped backslash and quote");
        }

        // Unknown escape sequence
        let expr = parse_expr(r#"msg == "unknown\x""#).unwrap();
        if let Expr::Compare {
            value: Value::String(s),
            ..
        } = expr
        {
            assert_eq!(s, "unknown\\x");
        } else {
            panic!("Expected string with literal backslash-x");
        }
    }

    #[test]
    fn parse_all_comparison_operators() {
        assert!(matches!(
            parse_expr("x == 1").unwrap(),
            Expr::Compare { op: CmpOp::Eq, .. }
        ));
        assert!(matches!(
            parse_expr("x != 1").unwrap(),
            Expr::Compare { op: CmpOp::Ne, .. }
        ));
        assert!(matches!(
            parse_expr("x > 1").unwrap(),
            Expr::Compare { op: CmpOp::Gt, .. }
        ));
        assert!(matches!(
            parse_expr("x >= 1").unwrap(),
            Expr::Compare { op: CmpOp::Ge, .. }
        ));
        assert!(matches!(
            parse_expr("x < 1").unwrap(),
            Expr::Compare { op: CmpOp::Lt, .. }
        ));
        assert!(matches!(
            parse_expr("x <= 1").unwrap(),
            Expr::Compare { op: CmpOp::Le, .. }
        ));
    }

    #[test]
    fn parse_negative_and_fractional_numbers() {
        let expr = parse_expr("x == -42").unwrap();
        if let Expr::Compare {
            value: Value::Number(n),
            ..
        } = expr
        {
            assert_eq!(n, -42.0);
        } else {
            panic!("Expected negative number");
        }

        let expr = parse_expr("x == 4.56").unwrap();
        if let Expr::Compare {
            value: Value::Number(n),
            ..
        } = expr
        {
            assert!((n - 4.56).abs() < 0.00001);
        } else {
            panic!("Expected fractional number");
        }

        let expr = parse_expr("x == -0.5").unwrap();
        if let Expr::Compare {
            value: Value::Number(n),
            ..
        } = expr
        {
            assert_eq!(n, -0.5);
        } else {
            panic!("Expected negative fractional number");
        }
    }

    #[test]
    fn parse_nested_parentheses() {
        let expr = parse_expr(r"((a == 1))").unwrap();
        assert!(matches!(expr, Expr::Compare { .. }));

        let expr = parse_expr(r"((a == 1 && b == 2) || (c == 3))").unwrap();
        assert!(matches!(expr, Expr::Or(_, _)));

        let expr = parse_expr(r"!(!(x == 1))").unwrap();
        if let Expr::Not(inner) = expr {
            assert!(matches!(*inner, Expr::Not(_)));
        } else {
            panic!("Expected nested Not");
        }
    }

    #[test]
    fn parse_operator_precedence() {
        // AND binds tighter than OR
        let expr = parse_expr(r"a == 1 || b == 2 && c == 3").unwrap();
        if let Expr::Or(left, right) = expr {
            assert!(matches!(*left, Expr::Compare { .. }));
            assert!(matches!(*right, Expr::And(_, _)));
        } else {
            panic!("Expected OR at top level with AND on right");
        }

        let expr = parse_expr(r"a == 1 && b == 2 || c == 3").unwrap();
        if let Expr::Or(left, _) = expr {
            assert!(matches!(*left, Expr::And(_, _)));
        } else {
            panic!("Expected OR at top level with AND on left");
        }
    }

    #[test]
    fn parse_error_messages() {
        let result = parse_expr("");
        assert!(result.is_err());
        if matches!(result, Err(QueryError::EmptyExpression)) {
            // Expected
        } else {
            panic!("Expected EmptyExpression error");
        }

        let result = parse_expr(r#"field == "unclosed"#);
        assert!(result.is_err());
        if let Err(QueryError::ParseError(_)) = result {
            // Expected
        } else {
            panic!("Expected ParseError for unclosed string");
        }

        let result = parse_expr("field ==");
        assert!(result.is_err());

        let result = parse_expr("== value");
        assert!(result.is_err());
    }

    #[test]
    fn parse_whitespace_handling() {
        let expr1 = parse_expr(r#"  field  ==  "value"  "#).unwrap();
        let expr2 = parse_expr(r#"field=="value""#).unwrap();
        assert_eq!(expr1, expr2);

        let expr = parse_expr("  a==1&&b==2  ").unwrap();
        assert!(matches!(expr, Expr::And(_, _)));
    }
}
