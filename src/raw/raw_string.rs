use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

use crate::{
    path::Path,
    raw::raw_value::{
        RAW_CONCAT_STRING_TYPE, RAW_MULTILINE_STRING_TYPE, RAW_QUOTED_STRING_TYPE,
        RAW_UNQUOTED_STRING_TYPE,
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum RawString {
    QuotedString(String),
    UnquotedString(String),
    MultilineString(String),
    ConcatString(ConcatString),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Constructor, Deref, DerefMut)]
pub struct ConcatString(Vec<(RawString, Option<String>)>);

impl ConcatString {
    pub fn synthetic(&self) -> String {
        let mut result = String::new();
        let iter = self.iter();
        let last_index = iter.len().saturating_sub(1);
        for (index, (string, space)) in iter.enumerate() {
            match string {
                RawString::ConcatString(s) => {
                    result.push_str(s.synthetic().as_str());
                }
                other => {
                    result.push_str(other.synthetic().as_str());
                }
            }
            if index != last_index
                && let Some(space) = space
            {
                result.push_str(space);
            }
        }
        result
    }

    pub fn merge(self) -> crate::Result<RawString> {
        Ok(RawString::quoted(self.to_string()))
    }
}

impl Display for ConcatString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let last_index = self.len().saturating_sub(1);
        for (index, (string, space)) in self.iter().enumerate() {
            write!(f, "{}", string)?;
            if index != last_index
                && let Some(space) = space
            {
                write!(f, " {}", space)?;
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
