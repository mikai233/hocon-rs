use crate::raw::raw_string::RawString;
use std::fmt::{Debug, Display, Formatter};

#[derive(Eq, PartialEq, Hash, Clone, derive_more::Constructor)]
pub struct Substitution {
    pub path: RawString,
    pub optional: bool,
}

impl Display for Substitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "${{")?;
        if self.optional {
            write!(f, "?")?;
        }
        write!(f, "{}", self.path)?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl Debug for Substitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "${{")?;
        if self.optional {
            write!(f, "?")?;
        }
        write!(f, "{:?}", self.path)?;
        write!(f, "}}")?;
        Ok(())
    }
}
