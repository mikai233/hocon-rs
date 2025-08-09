use crate::raw::raw_value::RawValue;
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(
    Debug,
    Clone,
    PartialEq,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::Constructor
)]
pub struct RawArray(Vec<RawValue>);

impl Display for RawArray {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().join(", "))
    }
}