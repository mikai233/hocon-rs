use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

#[derive(
    Debug,
    Clone,
    PartialEq,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::Constructor
)]
pub struct AddAssign(Box<RawValue>);

impl Display for AddAssign {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<RawValue> for AddAssign {
    fn into(self) -> RawValue {
        *self.0
    }
}