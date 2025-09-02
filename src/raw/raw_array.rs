use crate::{join, raw::raw_value::RawValue};
use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut, Constructor)]
pub struct RawArray(pub Vec<RawValue>);

impl RawArray {
    pub fn into_inner(self) -> Vec<RawValue> {
        self.0
    }
}

impl Display for RawArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        join(self.iter(), ", ", f)?;
        write!(f, "]")?;
        Ok(())
    }
}
