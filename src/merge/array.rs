use std::{cell::RefCell, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;

use crate::merge::vlaue::Value;

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct Array(pub(crate) Vec<RefCell<Value>>);

impl Array {
    pub(crate) fn is_merged(&self) -> bool {
        self.iter().all(|v| v.borrow().is_merged())
    }
}

impl TryFrom<crate::raw::raw_array::RawArray> for Array {
    type Error = crate::error::Error;

    fn try_from(value: crate::raw::raw_array::RawArray) -> Result<Self, Self::Error> {
        let values = value
            .0
            .into_iter()
            .map(|v| v.try_into())
            .collect::<crate::Result<Vec<Value>>>()?
            .into_iter()
            .map(|v| RefCell::new(v))
            .collect();
        Ok(Self::new(values))
    }
}

impl Display for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().map(|v| v.borrow()).join(", "))
    }
}
