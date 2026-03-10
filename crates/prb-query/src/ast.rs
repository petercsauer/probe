use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare {
        field: FieldPath,
        op: CmpOp,
        value: Value,
    },
    Contains {
        field: FieldPath,
        substring: String,
    },
    Exists {
        field: FieldPath,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldPath(pub Vec<String>);

impl FieldPath {
    pub fn dotted(&self) -> String {
        self.0.join(".")
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.dotted())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmpOp::Eq => write!(f, "=="),
            CmpOp::Ne => write!(f, "!="),
            CmpOp::Gt => write!(f, ">"),
            CmpOp::Ge => write!(f, ">="),
            CmpOp::Lt => write!(f, "<"),
            CmpOp::Le => write!(f, "<="),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Number(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_path_display() {
        let path = FieldPath(vec!["grpc".into(), "method".into()]);
        assert_eq!(path.to_string(), "grpc.method");
        assert_eq!(path.dotted(), "grpc.method");

        let single = FieldPath(vec!["transport".into()]);
        assert_eq!(single.to_string(), "transport");
    }

    #[test]
    fn cmp_op_display() {
        assert_eq!(CmpOp::Eq.to_string(), "==");
        assert_eq!(CmpOp::Ne.to_string(), "!=");
        assert_eq!(CmpOp::Gt.to_string(), ">");
        assert_eq!(CmpOp::Ge.to_string(), ">=");
        assert_eq!(CmpOp::Lt.to_string(), "<");
        assert_eq!(CmpOp::Le.to_string(), "<=");
    }

    #[test]
    fn value_display() {
        assert_eq!(Value::String("hello".into()).to_string(), r#""hello""#);
        assert_eq!(Value::Number(42.5).to_string(), "42.5");
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Bool(false).to_string(), "false");
    }

    #[test]
    fn expr_variants() {
        let compare = Expr::Compare {
            field: FieldPath(vec!["id".into()]),
            op: CmpOp::Eq,
            value: Value::Number(42.0),
        };
        assert!(matches!(compare, Expr::Compare { .. }));

        let contains = Expr::Contains {
            field: FieldPath(vec!["name".into()]),
            substring: "test".into(),
        };
        assert!(matches!(contains, Expr::Contains { .. }));

        let exists = Expr::Exists {
            field: FieldPath(vec!["optional".into()]),
        };
        assert!(matches!(exists, Expr::Exists { .. }));

        let not = Expr::Not(Box::new(compare.clone()));
        assert!(matches!(not, Expr::Not(_)));

        let and = Expr::And(Box::new(compare.clone()), Box::new(exists.clone()));
        assert!(matches!(and, Expr::And(_, _)));

        let or = Expr::Or(Box::new(compare.clone()), Box::new(exists));
        assert!(matches!(or, Expr::Or(_, _)));
    }
}
