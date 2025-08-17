use std::collections::VecDeque;

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::vlaue::Value;

#[derive(Debug, Clone, Deref, DerefMut, Constructor, PartialEq)]
pub(crate) struct Concat(pub(crate) VecDeque<Value>);

impl From<crate::raw::concat::Concat> for Concat {
    fn from(value: crate::raw::concat::Concat) -> Self {
        Self::new(value.0.into_iter().map(|v| v.into()).collect())
    }
}
