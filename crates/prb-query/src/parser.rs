use nom::Parser;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, multispace0},
    combinator::{map, opt, recognize},
    multi::separated_list1,
    sequence::preceded,
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

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_cont(c: char) -> bool {
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
    let (input, frac) =
        opt(preceded(char('.'), take_while1(|c: char| c.is_ascii_digit()))).parse(input)?;

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

fn atom(input: &str) -> IResult<&str, Expr> {
    if let Ok(result) = paren_expr(input) {
        return Ok(result);
    }
    if let Ok(result) = not_expr(input) {
        return Ok(result);
    }
    field_expr(input)
}

fn field_expr(input: &str) -> IResult<&str, Expr> {
    let (input, field) = ws(field_path).parse(input)?;

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
}
