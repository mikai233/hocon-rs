use derive_more::{Constructor, Deref, DerefMut};

use crate::{merge::vlaue::Value, raw::raw_value::RawValue};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct AddAssign(pub(crate) Box<Value>);

impl Into<Value> for AddAssign {
    fn into(self) -> Value {
        *self.0
    }
}

impl From<crate::raw::add_assign::AddAssign> for AddAssign {
    fn from(value: crate::raw::add_assign::AddAssign) -> Self {
        let raw: RawValue = value.into();
        let value: Value = raw.into();
        Self::new(value.into())
    }
}
