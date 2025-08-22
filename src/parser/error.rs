use nom_language::error::VerboseError;
use thiserror::Error;

use crate::parser::arena_input::ArenaInput;

#[derive(Debug, Error)]
pub enum ParseError<'a> {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(
        "Maximum inclusion depth reached for {0}. An inclusion cycle might have occurred. If not, try increasing `max_include_depth` in `ConfigOptions`."
    )]
    InclusionCycle(String),
    #[error("Inclusion not found: {0}")]
    InclusionNotFound(String),
    #[error("A cycle substitution found at {0}")]
    CycleSubstitution(String),
    #[error("{0}")]
    ConfigNotFound(String),
    #[error("{0}")]
    PropertiesParseError(#[from] java_properties::PropertiesError),
    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("{0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("{0}")]
    NomError(VerboseError<&'a str>),
}

impl<'a> nom::error::ParseError<ArenaInput<'a>> for ParseError<'a> {
    fn from_error_kind(input: ArenaInput<'a>, kind: nom::error::ErrorKind) -> Self {
        todo!()
    }

    fn append(input: ArenaInput<'a>, kind: nom::error::ErrorKind, other: Self) -> Self {
        todo!()
    }
}
