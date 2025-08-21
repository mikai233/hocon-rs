use std::{cell::RefCell, collections::VecDeque, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::{path::RefPath, value::Value};

#[derive(Debug, Clone, Deref, DerefMut, Constructor, PartialEq, Default)]
pub(crate) struct Concat(pub(crate) VecDeque<RefCell<Value>>);

impl Concat {
    pub(crate) fn from_iter<I>(values: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        let queue = VecDeque::from_iter(values.into_iter().map(RefCell::new));
        Self::new(queue)
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::concat::Concat,
    ) -> crate::Result<Self> {
        let mut values = VecDeque::with_capacity(raw.len());
        for val in raw.0 {
            let val = Value::from_raw(parent, val)?;
            values.push_back(RefCell::new(val));
        }
        Ok(Self::new(values))
    }
}

impl Display for Concat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Concat(")?;
        let last_index = self.len().saturating_sub(1);
        for (index, value) in self.iter().enumerate() {
            write!(f, "{}", value.borrow())?;
            if index != last_index {
                write!(f, ", ")?;
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}
