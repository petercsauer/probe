use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueryError {
    #[error("empty filter expression")]
    EmptyExpression,

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("unexpected trailing input: {0}")]
    TrailingInput(String),
}
