use prb_query::ast::{Expr, FieldPath, Value};
use prb_query::parser::parse_expr;

#[test]
fn test_parse_matches() {
    let expr = parse_expr(r#"tcp.payload matches "^GET""#).unwrap();
    assert!(matches!(
        expr,
        Expr::Matches {
            field: FieldPath(ref parts),
            pattern: ref p
        } if parts == &["tcp".to_string(), "payload".to_string()] && p == "^GET"
    ));
}

#[test]
fn test_parse_matches_complex_pattern() {
    let expr = parse_expr(r#"http.header matches "Content-Type: .*json""#).unwrap();
    assert!(matches!(expr, Expr::Matches { .. }));
}

#[test]
fn test_parse_in_numbers() {
    let expr = parse_expr("tcp.port in {80, 443, 8080}").unwrap();
    if let Expr::In { field, values } = expr {
        assert_eq!(field, FieldPath(vec!["tcp".into(), "port".into()]));
        assert_eq!(values.len(), 3);
        assert!(matches!(values[0], Value::Number(n) if (n - 80.0).abs() < f64::EPSILON));
        assert!(matches!(values[1], Value::Number(n) if (n - 443.0).abs() < f64::EPSILON));
        assert!(matches!(values[2], Value::Number(n) if (n - 8080.0).abs() < f64::EPSILON));
    } else {
        panic!("Expected In expression");
    }
}

#[test]
fn test_parse_in_strings() {
    let expr = parse_expr(r#"transport in {"tcp", "udp", "gRPC"}"#).unwrap();
    if let Expr::In { field, values } = expr {
        assert_eq!(field, FieldPath(vec!["transport".into()]));
        assert_eq!(values.len(), 3);
        assert!(matches!(&values[0], Value::String(s) if s == "tcp"));
        assert!(matches!(&values[1], Value::String(s) if s == "udp"));
        assert!(matches!(&values[2], Value::String(s) if s == "gRPC"));
    } else {
        panic!("Expected In expression");
    }
}

#[test]
fn test_parse_in_mixed() {
    let expr = parse_expr(r#"field in {1, "two", true}"#).unwrap();
    if let Expr::In { values, .. } = expr {
        assert_eq!(values.len(), 3);
        assert!(matches!(values[0], Value::Number(_)));
        assert!(matches!(values[1], Value::String(_)));
        assert!(matches!(values[2], Value::Bool(true)));
    } else {
        panic!("Expected In expression");
    }
}

#[test]
fn test_parse_slice() {
    let expr = parse_expr("tcp.payload[0:4]").unwrap();
    if let Expr::Slice { field, start, end } = expr {
        assert_eq!(field, FieldPath(vec!["tcp".into(), "payload".into()]));
        assert_eq!(start, 0);
        assert_eq!(end, 4);
    } else {
        panic!("Expected Slice expression");
    }
}

#[test]
fn test_parse_slice_different_range() {
    let expr = parse_expr("data[10:20]").unwrap();
    if let Expr::Slice { start, end, .. } = expr {
        assert_eq!(start, 10);
        assert_eq!(end, 20);
    } else {
        panic!("Expected Slice expression");
    }
}

#[test]
fn test_parse_slice_with_whitespace() {
    let expr = parse_expr("field[ 5 : 15 ]").unwrap();
    if let Expr::Slice { start, end, .. } = expr {
        assert_eq!(start, 5);
        assert_eq!(end, 15);
    } else {
        panic!("Expected Slice expression");
    }
}

#[test]
fn test_parse_function_len() {
    let expr = parse_expr("len(tcp.payload)").unwrap();
    if let Expr::Function { name, args } = expr {
        assert_eq!(name, "len");
        assert_eq!(args.len(), 1);
    } else {
        panic!("Expected Function expression");
    }
}

#[test]
fn test_parse_function_lower() {
    let expr = parse_expr("lower(http.host)").unwrap();
    if let Expr::Function { name, args } = expr {
        assert_eq!(name, "lower");
        assert_eq!(args.len(), 1);
    } else {
        panic!("Expected Function expression");
    }
}

#[test]
fn test_parse_function_upper() {
    let expr = parse_expr("upper(field)").unwrap();
    if let Expr::Function { name, .. } = expr {
        assert_eq!(name, "upper");
    } else {
        panic!("Expected Function expression");
    }
}

#[test]
fn test_parse_function_no_args() {
    let expr = parse_expr("now()").unwrap();
    if let Expr::Function { name, args } = expr {
        assert_eq!(name, "now");
        assert_eq!(args.len(), 0);
    } else {
        panic!("Expected Function expression");
    }
}

#[test]
fn test_parse_complex_filter_matches_and_in() {
    let expr = parse_expr(r#"tcp.port in {80, 443} && tcp.payload matches "^GET""#).unwrap();
    assert!(matches!(expr, Expr::And(_, _)));
}

#[test]
fn test_parse_complex_filter_with_slice() {
    let expr = parse_expr(r"data[0:4] && tcp.port == 80").unwrap();
    if let Expr::And(left, right) = expr {
        assert!(matches!(*left, Expr::Slice { .. }));
        assert!(matches!(*right, Expr::Compare { .. }));
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_parse_complex_filter_with_function() {
    let expr = parse_expr(r"len(payload) && status == 200").unwrap();
    if let Expr::And(left, right) = expr {
        assert!(matches!(*left, Expr::Function { .. }));
        assert!(matches!(*right, Expr::Compare { .. }));
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_parse_nested_function() {
    let expr = parse_expr("lower(upper(field))").unwrap();
    if let Expr::Function { name, args } = expr {
        assert_eq!(name, "lower");
        if let Expr::Function {
            name: inner_name, ..
        } = &**args.first().unwrap()
        {
            assert_eq!(inner_name, "upper");
        } else {
            panic!("Expected nested Function");
        }
    } else {
        panic!("Expected Function expression");
    }
}

#[test]
fn test_parse_or_with_matches() {
    let expr = parse_expr(r#"payload matches "error" || payload matches "warning""#).unwrap();
    assert!(matches!(expr, Expr::Or(_, _)));
}

#[test]
fn test_parse_not_with_in() {
    let expr = parse_expr(r"!port in {22, 23, 3389}").unwrap();
    if let Expr::Not(inner) = expr {
        assert!(matches!(*inner, Expr::In { .. }));
    } else {
        panic!("Expected Not expression");
    }
}

#[test]
fn test_parse_parentheses_with_new_operators() {
    let expr =
        parse_expr(r#"(payload matches "GET" || payload matches "POST") && port in {80, 443}"#)
            .unwrap();
    if let Expr::And(left, right) = expr {
        assert!(matches!(*left, Expr::Or(_, _)));
        assert!(matches!(*right, Expr::In { .. }));
    } else {
        panic!("Expected And expression with Or on left");
    }
}

#[test]
fn test_parse_function_with_multiple_args() {
    let expr = parse_expr("concat(field1, field2)").unwrap();
    if let Expr::Function { name, args } = expr {
        assert_eq!(name, "concat");
        assert_eq!(args.len(), 2);
    } else {
        panic!("Expected Function expression with 2 args");
    }
}

#[test]
fn test_parse_empty_in_set_fails() {
    let result = parse_expr("field in {}");
    assert!(result.is_err());
}

#[test]
fn test_parse_single_value_in_set() {
    let expr = parse_expr("field in {42}").unwrap();
    if let Expr::In { values, .. } = expr {
        assert_eq!(values.len(), 1);
    } else {
        panic!("Expected In expression");
    }
}
