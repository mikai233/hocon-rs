use std::fmt::Display;

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
    },
    #[error("{val} is not allowed in {ty}")]
    InvalidValue { val: &'static str, ty: &'static str },
    #[error("substitution {0} not found")]
    SubstitutionNotFound(String),
    #[error(
        "Substitution incomplete. This should never happen outside this library. If you see this, it's a bug."
    )]
    SubstitutionNotComplete,
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
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
