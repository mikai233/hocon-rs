use nom::error::{ContextError, FromExternalError};
use nom_language::error::VerboseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HoconParseError<'a> {
    #[error("{0}")]
    Nom(VerboseError<&'a str>),
    #[error("{0}")]
    Other(#[from] crate::error::Error)
}

impl<'a> HoconParseError<'a> {
    pub fn as_nom_error_mut(&mut self) -> Option<&mut VerboseError<&'a str>> {
        if let HoconParseError::Nom(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl<'a> nom::error::ParseError<&'a str> for HoconParseError<'a> {
    fn from_error_kind(input: &'a str, kind: nom::error::ErrorKind) -> Self {
        let verbose_error = VerboseError {
            errors: vec![(input, nom_language::error::VerboseErrorKind::Nom(kind))],
        };
        Self::Nom(verbose_error)
    }

    fn append(input: &'a str, kind: nom::error::ErrorKind, mut other: Self) -> Self {
        if let Some(e) = other.as_nom_error_mut() {
            e.errors
                .push((input, nom_language::error::VerboseErrorKind::Nom(kind)));
        }
        other
    }

    fn from_char(input: &'a str, c: char) -> Self {
        let verbose_error = VerboseError {
            errors: vec![(input, nom_language::error::VerboseErrorKind::Char(c))],
        };
        Self::Nom(verbose_error)
    }
}

impl<'a> ContextError<&'a str> for HoconParseError<'a> {
    fn add_context(input: &'a str, ctx: &'static str, mut other: Self) -> Self {
        if let Some(e) = other.as_nom_error_mut() {
            e.errors
                .push((input, nom_language::error::VerboseErrorKind::Context(ctx)));
        }
        other
    }
}

impl<'a, E> FromExternalError<&'a str, E> for HoconParseError<'a> {
    fn from_external_error(input: &'a str, kind: nom::error::ErrorKind, _e: E) -> Self {
        <Self as nom::error::ParseError<&'a str>>::from_error_kind(input, kind)
    }
}
