use std::collections::VecDeque;

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::vlaue::Value;

/// There are some some substitutions during the replacement operation,
/// we can not known them at this time, so we construct a `DelayMerge`
/// struct to merge it in the future.
/// We don't know the merge result, because we don't no whether the substitutions
/// is simple value or object value.
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct DelayMerge(pub(crate) VecDeque<Value>);

impl DelayMerge {
    pub(crate) fn from_iter<I>(value: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        Self::new(value.into_iter().collect())
    }

    pub(crate) fn into_values(self) -> VecDeque<Value> {
        self.0
    }
}

impl DelayMerge {}
