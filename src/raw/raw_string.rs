use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::{Debug, Display, Formatter};

use crate::{
    join, join_debug,
    path::Path,
    raw::raw_value::{
        RAW_CONCAT_STRING_TYPE, RAW_MULTILINE_STRING_TYPE, RAW_QUOTED_STRING_TYPE,
        RAW_UNQUOTED_STRING_TYPE,
    },
};

/// Represents the different types of string values in a HOCON configuration.
///
/// This enum covers the three standard HOCON string types, plus an additional variant
/// to handle path expressions.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum RawString {
    /// A string literal enclosed in double quotes.
    QuotedString(String),
    /// A simple string without quotes.
    UnquotedString(String),
    /// A multiline string enclosed in three double quotes.
    MultilineString(String),
    /// A path expression
    PathExpression(PathExpression),
}

#[derive(Clone, Eq, PartialEq, Hash, Constructor, Deref, DerefMut)]
pub struct PathExpression(Vec<RawString>);

impl PathExpression {
    pub fn into_inner(self) -> Vec<RawString> {
        self.0
    }
}

impl Debug for PathExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        join_debug(self.iter(), ".", f)
    }
}

impl Display for PathExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        join(self.iter(), ".", f)
    }
}

impl Into<RawString> for &str {
    fn into(self) -> RawString {
        if self.chars().any(|c| c == '\n') {
            RawString::multiline(self)
        } else {
            RawString::quoted(self)
        }
    }
}

impl Into<RawString> for String {
    fn into(self) -> RawString {
        if self.chars().any(|c| c == '\n') {
            RawString::multiline(self)
        } else {
            RawString::quoted(self)
        }
    }
}

impl RawString {
    pub fn ty(&self) -> &'static str {
        match self {
            RawString::QuotedString(_) => RAW_QUOTED_STRING_TYPE,
            RawString::UnquotedString(_) => RAW_UNQUOTED_STRING_TYPE,
            RawString::MultilineString(_) => RAW_MULTILINE_STRING_TYPE,
            RawString::PathExpression(_) => RAW_CONCAT_STRING_TYPE,
        }
    }

    pub fn as_path(&self) -> Vec<&str> {
        match self {
            RawString::QuotedString(s)
            | RawString::UnquotedString(s)
            | RawString::MultilineString(s) => vec![s],
            RawString::PathExpression(c) => c.iter().flat_map(|s| s.as_path()).collect(),
        }
    }

    pub fn into_path(self) -> Path {
        match self {
            RawString::QuotedString(s)
            | RawString::UnquotedString(s)
            | RawString::MultilineString(s) => Path::new(s, None),
            RawString::PathExpression(c) => {
                let mut dummy = Path::new("".to_string(), None);
                let mut curr = &mut dummy;
                for path in c.into_inner() {
                    curr.remainder = Some(Box::new(path.into_path()));
                    curr = curr.remainder.as_mut().unwrap();
                }
                *dummy.remainder.expect("empty path found")
            }
        }
    }

    pub fn quoted(string: impl Into<String>) -> Self {
        Self::QuotedString(string.into())
    }

    pub fn unquoted(string: impl Into<String>) -> Self {
        Self::UnquotedString(string.into())
    }

    pub fn multiline(string: impl Into<String>) -> Self {
        Self::MultilineString(string.into())
    }

    pub fn path_expression(paths: Vec<RawString>) -> Self {
        Self::PathExpression(PathExpression::new(paths))
    }
}

impl Display for RawString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawString::QuotedString(s) => write!(f, "{}", s),
            RawString::UnquotedString(s) => write!(f, "{}", s),
            RawString::MultilineString(s) => write!(f, "{}", s),
            RawString::PathExpression(s) => write!(f, "{}", s),
        }
    }
}

impl Debug for RawString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuotedString(s) => {
                write!(f, "\"{:?}\"", s)
            }
            Self::UnquotedString(s) => {
                write!(f, "{:?}", s)
            }
            Self::MultilineString(s) => {
                write!(f, "\"\"\"{:?}\"\"\"", s)
            }
            Self::PathExpression(s) => {
                write!(f, "{:?}", s)
            }
        }
    }
}
