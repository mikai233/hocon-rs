use std::fmt::Display;

use derive_more::{Constructor, Deref, DerefMut};

use crate::Result;
use crate::{
    merge::{path::RefPath, value::Value},
    raw::raw_value::RawValue,
};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor, Default)]
pub(crate) struct AddAssign(pub(crate) Box<Value>);

impl AddAssign {
    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::add_assign::AddAssign,
    ) -> crate::Result<Self> {
        let raw: RawValue = raw.into();
        let value = Value::from_raw(parent, raw)?;
        Ok(Self::new(value.into()))
    }

    pub(crate) fn try_resolve(self, path: &RefPath) -> Result<Value> {
        let value = if self.is_merged() {
            *self.0
        } else {
            match *self.0 {
                Value::Concat(concat) => concat.try_resolve(path)?,
                other => other,
            }
        };
        Ok(value)
    }
}

impl Into<Value> for AddAssign {
    fn into(self) -> Value {
        *self.0
    }
}

impl Display for AddAssign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "+={}", self.0)
    }
}
