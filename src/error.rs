#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot convert `{from}` to `{to}`")]
    InvalidConversion {
        from: &'static str,
        to: &'static str,
    },
    #[error("cannot convert `{from}` to `{to}`")]
    PrecisionLoss {
        from: &'static str,
        to: &'static str,
    },
    #[error("invalid path expression: {0}")]
    InvalidPathExpression(&'static str),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("cannot concatenation different type: {ty1} {ty2}")]
    ConcatenationDifferentType {
        ty1: &'static str,
        ty2: &'static str,
    }
}