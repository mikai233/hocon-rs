use std::fmt::Display;

use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;

use crate::merge::vlaue::Value;

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct Array(pub(crate) Vec<Value>);

impl Array {
    pub(crate) fn is_merged(&self) -> bool {
        self.iter().all(Value::is_merged)
    }
}

impl From<crate::raw::raw_array::RawArray> for Array {
    fn from(value: crate::raw::raw_array::RawArray) -> Self {
        Self::new(value.0.into_iter().map(|v| v.into()).collect())
    }
}

impl Display for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().join(", "))
    }
}
