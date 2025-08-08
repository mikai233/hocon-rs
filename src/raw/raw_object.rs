use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Default, Deref, DerefMut, Constructor)]
pub struct RawObject(Vec<ObjectField>);

impl RawObject {
    pub fn kv<I>(iter: I) -> Self
    where
        I: IntoIterator<Item=(RawString, RawValue)>,
    {
        let kvs = iter
            .into_iter()
            .map(|(k, v)| ObjectField::KeyValue(k, v))
            .collect();
        Self(kvs)
    }
}

impl Display for RawObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let joined = self.iter()
            .map(|v| format!("{}", v))
            .join(", ");
        write!(f, "{{{}}}", joined)
    }
}

impl From<Vec<(String, RawValue)>> for RawObject {
    fn from(value: Vec<(String, RawValue)>) -> Self {
        let fields = value.into_iter().map(|(k, v)| ObjectField::KeyValue(RawString::QuotedString(k), v)).collect();
        Self(fields)
    }
}
