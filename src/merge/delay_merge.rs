use std::{cell::RefCell, collections::VecDeque, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::vlaue::Value;

/// There are some some substitutions during the replacement operation,
/// we can not known them at this time, so we construct a `DelayMerge`
/// struct to merge it in the future.
/// We don't know the merge result, because we don't no whether the substitutions
/// is simple value or object value.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct DelayMerge(pub(crate) VecDeque<RefCell<Value>>);

impl DelayMerge {
    pub(crate) fn from_iter<I>(value: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        Self::new(value.into_iter().map(|v| RefCell::new(v)).collect())
    }

    pub(crate) fn into_values(self) -> VecDeque<RefCell<Value>> {
        self.0
    }
}

impl DelayMerge {}

impl Display for DelayMerge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DelayMerge(")?;
        let last_index = self.len().saturating_sub(1);
        for (index, ele) in self.iter().enumerate() {
            write!(f, "{}", ele.borrow())?;
            if index != last_index {
                write!(f, ", ")?;
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}
