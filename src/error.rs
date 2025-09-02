use std::fmt::Display;

use crate::raw::include::Inclusion;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Serde(#[from] serde_json::Error),
    #[error("Invalid escape")]
    InvalidEscape,
    #[error(
        "Unexpected token, expected:{}, found beginning:{}",
        expected,
        found_beginning
    )]
    UnexpectedToken {
        expected: &'static str,
        found_beginning: char,
    },
    #[error("End of file")]
    Eof,
    #[error("Cannot convert `{from}` to `{to}`")]
    InvalidConversion {
        from: &'static str,
        to: &'static str,
    },
    #[error("Invalid path expression: {0}")]
    InvalidPathExpression(&'static str),
    #[error(
        "Cannot concatenation different type {left_ty}:{left} and {right_ty}:{right} at {path}"
    )]
    ConcatenationDifferentType {
        path: String,
        left: String,
        left_ty: &'static str,
        right: String,
        right_ty: &'static str,
    },
    #[error("{val} is not allowed in {ty}")]
    InvalidValue { val: &'static str, ty: &'static str },
    #[error("Substitution {0} not found")]
    SubstitutionNotFound(String),
    #[error(
        "Resolve incomplete. This should never happen outside this library. If you see this, it's a bug."
    )]
    ResolveNotComplete,
    #[error(
        "Maximum inclusion depth reached for {0}. An inclusion cycle might have occurred. If not, try increasing `max_include_depth` in `ConfigOptions`."
    )]
    InclusionCycle(String),
    #[error("Object nesting depth exceeded the limit of {max_depth} levels")]
    RecursionDepthExceeded { max_depth: u32 },
    #[error("Inclusion: {inclusion} error: {error}")]
    Include {
        inclusion: Inclusion,
        error: Box<Error>,
    },
    #[error("A cycle substitution found at {0}")]
    CycleSubstitution(String),
    #[error("{0}")]
    DeserializeError(String),
    #[error("{0}")]
    PropertiesParseError(#[from] java_properties::PropertiesError),
    #[error("{0}")]
    UrlParseError(#[from] url::ParseError),
}

impl Error {
    pub fn unexpected_token(expected: &'static str, found_beginning: char) -> Self {
        Self::UnexpectedToken {
            expected,
            found_beginning,
        }
    }
}

impl serde::de::Error for Error {
    #[doc = r" Raised when there is general error when deserializing a type."]
    #[doc = r""]
    #[doc = r" The message should not be capitalized and should not end with a period."]
    #[doc = r""]
    #[doc = r" ```edition2021"]
    #[doc = r" # use std::str::FromStr;"]
    #[doc = r" #"]
    #[doc = r" # struct IpAddr;"]
    #[doc = r" #"]
    #[doc = r" # impl FromStr for IpAddr {"]
    #[doc = r" #     type Err = String;"]
    #[doc = r" #"]
    #[doc = r" #     fn from_str(_: &str) -> Result<Self, String> {"]
    #[doc = r" #         unimplemented!()"]
    #[doc = r" #     }"]
    #[doc = r" # }"]
    #[doc = r" #"]
    #[doc = r" use serde::de::{self, Deserialize, Deserializer};"]
    #[doc = r""]
    #[doc = r" impl<'de> Deserialize<'de> for IpAddr {"]
    #[doc = r"     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>"]
    #[doc = r"     where"]
    #[doc = r"         D: Deserializer<'de>,"]
    #[doc = r"     {"]
    #[doc = r"         let s = String::deserialize(deserializer)?;"]
    #[doc = r"         s.parse().map_err(de::Error::custom)"]
    #[doc = r"     }"]
    #[doc = r" }"]
    #[doc = r" ```"]
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::DeserializeError(msg.to_string())
    }
}
