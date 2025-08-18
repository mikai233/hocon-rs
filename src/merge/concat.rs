use std::{cell::RefCell, collections::VecDeque, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::vlaue::Value;

#[derive(Debug, Clone, Deref, DerefMut, Constructor, PartialEq, Default)]
pub(crate) struct Concat(pub(crate) VecDeque<RefCell<Value>>);

impl Concat {
    pub(crate) fn reslove(self) -> crate::Result<Value> {
        self.0.into_iter().try_fold(Value::Null, |left, right| {
            Value::concatenate(left, right.into_inner())
        })
    }

    pub(crate) fn from_iter<I>(values: I) -> Self
    where
        I: IntoIterator<Item=Value>,
    {
        let queue = VecDeque::from_iter(values.into_iter().map(RefCell::new));
        Self::new(queue)
    }
}

impl TryFrom<crate::raw::concat::Concat> for Concat {
    type Error = crate::error::Error;

    fn try_from(value: crate::raw::concat::Concat) -> Result<Self, Self::Error> {
        let values = value
            .0
            .into_iter()
            .map(|v| v.try_into())
            .collect::<crate::Result<VecDeque<Value>>>()?
            .into_iter()
            .map(|v| RefCell::new(v))
            .collect();
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
