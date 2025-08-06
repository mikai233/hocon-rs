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
pub struct Concat(Vec<Vec<RawValue>>);

impl Display for Concat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for values in self.iter() {
            write!(f, "[{}],", values.iter().join(", "))?;
        }
        write!(f, "]")?;
        Ok(())
    }
}