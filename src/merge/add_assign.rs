use std::fmt::Display;

use derive_more::{Constructor, Deref, DerefMut};

use crate::{merge::value::Value, raw::raw_value::RawValue};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor, Default)]
pub(crate) struct AddAssign(pub(crate) Box<Value>);

impl Into<Value> for AddAssign {
    fn into(self) -> Value {
        *self.0
    }
}

impl TryFrom<crate::raw::add_assign::AddAssign> for AddAssign {
    type Error = crate::error::Error;

    fn try_from(value: crate::raw::add_assign::AddAssign) -> Result<Self, Self::Error> {
        let raw: RawValue = value.into();
        let value: Value = raw.try_into()?;
        Ok(Self::new(value.into()))
    }
}

impl Display for AddAssign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "+={}", self.0)
    }
}
