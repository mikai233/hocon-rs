use std::{
    cell::RefCell,
    fmt::Display,
    ops::{Deref, DerefMut},
};

use tracing::trace;

use crate::merge::{path::RefPath, value::Value};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Array {
    Merged(Vec<RefCell<Value>>),
    Unmerged(Vec<RefCell<Value>>),
}

impl Array {
    pub(crate) fn new(values: Vec<RefCell<Value>>) -> Self {
        Array::Unmerged(values)
    }

    pub(crate) fn is_merged(&self) -> bool {
        matches!(self, Array::Merged(_))
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::raw_array::RawArray,
    ) -> crate::Result<Self> {
        let mut values = Vec::with_capacity(raw.len());
        for val in raw.into_inner() {
            let val = Value::from_raw(parent, val)?;
            values.push(RefCell::new(val));
        }
        Ok(Self::new(values))
    }

    pub(crate) fn as_merged(&mut self) {
        let array = std::mem::take(self.deref_mut());
        *self = Self::Merged(array);
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        if self.is_merged() {
            return true;
        }
        let all_merged = self.iter_mut().all(|v| v.get_mut().try_become_merged());
        if all_merged {
            self.as_merged();
            trace!("{} become merged", self);
        }
        all_merged
    }

    pub(crate) fn into_inner(self) -> Vec<RefCell<Value>> {
        match self {
            Array::Merged(array) | Array::Unmerged(array) => array,
        }
    }
}

impl Deref for Array {
    type Target = Vec<RefCell<Value>>;

    fn deref(&self) -> &Self::Target {
        match self {
            Array::Merged(array) | Array::Unmerged(array) => array,
        }
    }
}

impl DerefMut for Array {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Array::Merged(array) | Array::Unmerged(array) => array,
        }
    }
}

impl Display for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.iter();
        write!(f, "[")?;
        match iter.next() {
            Some(v) => {
                write!(f, "{}", v.borrow())?;
                for v in iter {
                    write!(f, ", ")?;
                    write!(f, "{}", v.borrow())?;
                }
            }
            None => {}
        }
        write!(f, "]")?;
        Ok(())
    }
}
