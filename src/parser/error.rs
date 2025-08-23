use nom::error::{ContextError, FromExternalError};
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
    NomError(VerboseError<ArenaInput<'a>>),
}

impl<'a> ParseError<'a> {
    pub fn as_nom_error_mut(&mut self) -> Option<&mut VerboseError<ArenaInput<'a>>> {
        if let ParseError::NomError(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl<'a> nom::error::ParseError<ArenaInput<'a>> for ParseError<'a> {
    fn from_error_kind(input: ArenaInput<'a>, kind: nom::error::ErrorKind) -> Self {
        let verbose_error = VerboseError {
            errors: vec![(input, nom_language::error::VerboseErrorKind::Nom(kind))],
        };
        Self::NomError(verbose_error)
    }

    fn append(input: ArenaInput<'a>, kind: nom::error::ErrorKind, mut other: Self) -> Self {
        if let Some(e) = other.as_nom_error_mut() {
            e.errors
                .push((input, nom_language::error::VerboseErrorKind::Nom(kind)));
        }
        other
    }

    fn from_char(input: ArenaInput<'a>, c: char) -> Self {
        let verbose_error = VerboseError {
            errors: vec![(input, nom_language::error::VerboseErrorKind::Char(c))],
        };
        Self::NomError(verbose_error)
    }
}

impl<'a> ContextError<ArenaInput<'a>> for ParseError<'a> {
    fn add_context(input: ArenaInput<'a>, ctx: &'static str, mut other: Self) -> Self {
        if let Some(e) = other.as_nom_error_mut() {
            e.errors
                .push((input, nom_language::error::VerboseErrorKind::Context(ctx)));
        }
        other
    }
}

impl<'a, E> FromExternalError<ArenaInput<'a>, E> for ParseError<'a> {
    fn from_external_error(input: ArenaInput<'a>, kind: nom::error::ErrorKind, _e: E) -> Self {
        <Self as nom::error::ParseError<ArenaInput>>::from_error_kind(input, kind)
    }
}
