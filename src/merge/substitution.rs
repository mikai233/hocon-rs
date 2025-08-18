use std::fmt::{Display, Formatter};

use derive_more::Constructor;

use crate::path::Path;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Constructor)]
pub(crate) struct Substitution {
    pub(crate) path: Path,
    pub(crate) optional: bool,
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

impl From<crate::raw::substitution::Substitution> for Substitution {
    fn from(value: crate::raw::substitution::Substitution) -> Self {
        Self::new(value.path.into_path(), value.optional)
    }
}
