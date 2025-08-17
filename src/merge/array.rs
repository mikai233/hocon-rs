use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::vlaue::Value;

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct Array(pub(crate) Vec<Value>);

impl From<crate::raw::raw_array::RawArray> for Array {
    fn from(value: crate::raw::raw_array::RawArray) -> Self {
        Self::new(value.0.into_iter().map(|v| v.into()).collect())
    }
}
