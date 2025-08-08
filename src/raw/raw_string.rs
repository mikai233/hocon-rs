use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum RawString {
    QuotedString(String),
    UnquotedString(String),
    MultiLineString(String),
    ConcatString(ConcatString),
}

#[derive(Debug, Clone, PartialEq, Hash, Constructor, Deref, DerefMut)]
pub struct ConcatString(Vec<(RawString, String)>);

impl ConcatString {
    pub fn synthetic(&self) -> String {
        let mut result = String::new();
        let iter = self.iter();
        let last_index = iter.len();
        for (index, (string, space)) in iter.enumerate() {
            match string {
                RawString::QuotedString(s) |
                RawString::UnquotedString(s) |
                RawString::MultiLineString(s) => {
                    result.push_str(s.as_str());
                }
                RawString::ConcatString(s) => {
                    result.push_str(s.synthetic().as_str());
                }
            }
            if index != last_index && !space.is_empty() {
                result.push_str(space);
            }
        }
        result
    }
}

impl Display for ConcatString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (string, space) in self.iter() {
            write!(f, "{}{}", string, space)?;
        }
        Ok(())
    }
}

impl RawString {
    pub fn ty(&self) -> &'static str {
        match self {
            RawString::QuotedString(_) => "quoted_string",
            RawString::UnquotedString(_) => "unquoted_string",
            RawString::MultiLineString(_) => "multi_line_string",
            RawString::ConcatString(_) => "concat_string",
        }
    }

    pub fn synthetic(self) -> String {
        match self {
            RawString::QuotedString(s) |
            RawString::UnquotedString(s) |
            RawString::MultiLineString(s) => s,
            RawString::ConcatString(s) => s.synthetic(),
        }
    }

    pub fn quoted(string: impl Into<String>) -> Self {
        Self::QuotedString(string.into())
    }

    pub fn unquoted(string: impl Into<String>) -> Self {
        Self::UnquotedString(string.into())
    }

    pub fn multi_line(string: impl Into<String>) -> Self {
        Self::MultiLineString(string.into())
    }

    pub fn concat<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item=(RawString, S)>,
        S: Into<String>,
    {
        let strings = iter.into_iter().map(|(t, u)| (t, u.into())).collect_vec();
        Self::ConcatString(ConcatString::new(strings))
    }
}

impl Display for RawString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawString::QuotedString(s) => write!(f, "{}", s),
            RawString::UnquotedString(s) => write!(f, "{}", s),
            RawString::MultiLineString(s) => write!(f, "{}", s),
            RawString::ConcatString(s) => write!(f, "{:?}", s),
        }
    }
}