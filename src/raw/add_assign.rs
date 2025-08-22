use derive_more::{Constructor, Deref, DerefMut};

use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut, Constructor)]
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
