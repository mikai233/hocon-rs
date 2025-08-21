use std::{cell::RefCell, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;

use crate::merge::{path::RefPath, value::Value};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct Array(pub(crate) Vec<RefCell<Value>>);

impl Array {
    pub(crate) fn is_merged(&self) -> bool {
        self.iter().all(|v| v.borrow().is_merged())
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::raw_array::RawArray,
    ) -> crate::Result<Self> {
        let mut values = Vec::with_capacity(raw.len());
        for val in raw.0 {
            let val = Value::from_raw(parent, val)?;
            values.push(RefCell::new(val));
        }
        Ok(Self::new(values))
    }
}

impl Display for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().map(|v| v.borrow()).join(", "))
    }
}
