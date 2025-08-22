use crate::raw::raw_value::RawValue;
use derive_more::{Deref, DerefMut};
use itertools::Itertools;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct Concat(pub(crate) Vec<RawValue>);

impl Concat {
    pub fn new<I>(values: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = RawValue>,
    {
        let concat = Self(values.into_iter().collect());
        for v in &concat.0 {
            if matches!(v, RawValue::Concat(_)) || matches!(v, RawValue::AddAssign(_)) {
                return Err(crate::error::Error::InvalidValue {
                    val: v.ty(),
                    ty: "concat",
                });
            }
        }
        Ok(concat)
    }
}

impl Display for Concat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.iter().join(" "))
    }
}
