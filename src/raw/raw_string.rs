use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Debug, Display, Formatter};

use crate::{
    path::Path,
    raw::raw_value::{
        RAW_CONCAT_STRING_TYPE, RAW_MULTILINE_STRING_TYPE, RAW_QUOTED_STRING_TYPE,
        RAW_UNQUOTED_STRING_TYPE,
    },
};

/// Represents the different types of string values in a HOCON configuration.
///
/// This enum covers the three standard HOCON string types, plus an additional variant
/// to handle string concatenations and path expressions.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum RawString {
    /// A string literal enclosed in double quotes.
    QuotedString(String),
    /// A simple string without quotes.
    UnquotedString(String),
    /// A multiline string enclosed in three double quotes.
    MultilineString(String),
    /// Represents a sequence of strings that are implicitly concatenated.
    /// This is also used to represent path expressions.
    /// FIXME change this to PathExpression, and the string concat should
    /// use the [crate::raw::concat::Concat] struct
    ConcatString(ConcatString),
}

/// A structure to handle string concatenations and path expressions.
///
/// It holds a vector of string fragments and optional separators.
/// This allows the parser to represent `HOCON`'s string concatenation
/// (`"hello" "world"`) and path expressions (`a.b.c`) in a unified way,
/// preserving the individual segments.
#[derive(Clone, Eq, PartialEq, Hash, Constructor, Deref, DerefMut)]
pub struct ConcatString(Vec<(RawString, Option<String>)>);

impl ConcatString {
    /// Synthesizes the concatenated string fragments into a single `String`.
    ///
    /// This method recursively processes any nested `ConcatString`s and
    /// joins the fragments, inserting separators (like a dot in a path)
    /// if they exist.
    pub fn synthetic(&self) -> String {
        let mut result = String::new();
        for (string, space) in self.iter() {
            match string {
                RawString::ConcatString(s) => {
                    result.push_str(s.synthetic().as_str());
                }
                other => {
                    result.push_str(other.synthetic().as_str());
                }
            }
            if let Some(space) = space {
                result.push_str(space);
            }
        }
        result
    }

    /// Merges the `ConcatString` into a single `RawString::QuotedString`.
    ///
    /// This is the final step in resolving a concatenated string, providing a simple,
    /// single-value representation for further use.
    pub fn merge(self) -> crate::Result<RawString> {
        Ok(RawString::quoted(self.to_string()))
    }
}

impl Debug for ConcatString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (string, space) in self.iter() {
            match string {
                RawString::ConcatString(s) => {
                    write!(f, "{:?}", s)?;
                }
                other => {
                    write!(f, "{:?}", other)?;
                }
            }
            if let Some(space) = space {
                write!(f, "{}", space)?;
            }
        }
        Ok(())
    }
}

impl Display for ConcatString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (string, space) in self.iter() {
            write!(f, "{}", string)?;
            if let Some(space) = space {
                write!(f, "{}", space)?;
            }
        }
        Ok(())
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
            RawString::ConcatString(_) => RAW_CONCAT_STRING_TYPE,
        }
    }

    pub fn synthetic(&self) -> String {
        let mut result = String::new();
        match self {
            RawString::QuotedString(s) => {
                result.push('"');
                result.push_str(s);
                result.push('"');
            }
            RawString::UnquotedString(s) => {
                result.push_str(s);
            }
            RawString::MultilineString(s) => {
                result.push_str("\"\"\"");
                result.push_str(s);
                result.push_str("\"\"\"");
            }
            RawString::ConcatString(s) => {
                result = s.synthetic();
            }
        }
        result
    }

    pub fn as_path(&self) -> Vec<&str> {
        match self {
            RawString::QuotedString(s)
            | RawString::UnquotedString(s)
            | RawString::MultilineString(s) => vec![s],
            RawString::ConcatString(c) => c.iter().flat_map(|(s, _)| s.as_path()).collect(),
        }
    }

    pub fn into_path(self) -> Path {
        match self {
            RawString::QuotedString(s)
            | RawString::UnquotedString(s)
            | RawString::MultilineString(s) => Path::new(s, None),
            RawString::ConcatString(c) => {
                let mut dummy = Path::new("".to_string(), None);
                let mut curr = &mut dummy;
                for (path, _) in c.0.into_iter() {
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

    pub fn concat<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = (RawString, Option<S>)>,
        S: Into<String>,
    {
        let strings = iter
            .into_iter()
            .map(|(t, u)| (t, u.map(|u| u.into())))
            .collect_vec();
        Self::ConcatString(ConcatString::new(strings))
    }
}

impl Display for RawString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawString::QuotedString(s) => write!(f, "{}", s),
            RawString::UnquotedString(s) => write!(f, "{}", s),
            RawString::MultilineString(s) => write!(f, "{}", s),
            RawString::ConcatString(s) => write!(f, "{}", s),
        }
    }
}

impl Debug for RawString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuotedString(s) => {
                write!(f, "\"{}\"", s)
            }
            Self::UnquotedString(s) => {
                write!(f, "{}", s)
            }
            Self::MultilineString(s) => {
                write!(f, "\"\"\"{}\"\"\"", s)
            }
            Self::ConcatString(s) => {
                write!(f, "{:?}", s)
            }
        }
    }
}
