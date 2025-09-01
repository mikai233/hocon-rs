use crate::raw::raw_string::RawString;
use std::fmt::{Debug, Display, Formatter};

#[derive(Eq, PartialEq, Hash, Clone, derive_more::Constructor)]
pub struct Substitution {
    pub path: RawString,
    pub optional: bool,
    pub space: Option<String>, // Space after substitution expression is necessary when the substitute result is string
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
        write!(f, "{}", self.path.synthetic())?;
        write!(f, "}}")?;
        Ok(())
    }
}
