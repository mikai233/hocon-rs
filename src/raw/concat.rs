use crate::{error::Error, join, raw::raw_value::RawValue};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Concat {
    values: Vec<RawValue>,
    spaces: Vec<Option<String>>,
}

impl Concat {
    pub fn new(values: Vec<RawValue>, spaces: Vec<Option<String>>) -> crate::Result<Self> {
        if values.len() != spaces.len() + 1 {
            return Err(Error::InvalidConcat(values.len(), spaces.len()));
        }
        let concat = Self { values, spaces };
        for v in &concat.values {
            if matches!(v, RawValue::Concat(_)) || matches!(v, RawValue::AddAssign(_)) {
                return Err(Error::InvalidValue {
                    val: v.ty(),
                    ty: "concat",
                });
            }
        }
        Ok(concat)
    }

    pub fn into_inner(self) -> (Vec<RawValue>, Vec<Option<String>>) {
        (self.values, self.spaces)
    }

    pub fn get_values(&self) -> &Vec<RawValue> {
        &self.values
    }

    pub fn get_spaces(&self) -> &Vec<Option<String>> {
        &self.spaces
    }
}

impl Display for Concat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        join(self.values.iter(), " ", f)
    }
}
