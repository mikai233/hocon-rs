use crate::raw::raw_value::RawValue;
use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Constructor, Deref, DerefMut)]
pub struct Concat(Vec<RawValue>);

impl Display for Concat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.iter().join(" "))
    }
}