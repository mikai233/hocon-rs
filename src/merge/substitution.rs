use std::{
    fmt::{Display, Formatter},
    rc::Rc,
};

use derive_more::Constructor;

use crate::path::Path;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Constructor)]
pub(crate) struct Substitution {
    pub(crate) path: Rc<Path>,
    pub(crate) optional: bool,
}

impl Substitution {
    pub(crate) fn full_path(&self) -> String {
        self.path.iter().fold(String::new(), |mut acc, next| {
            acc.push_str(&next.first);
            if next.remainder.is_some() {
                acc.push('.');
            }
            acc
        })
    }
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
        let path = value.path.into_path().into();
        Self::new(path, value.optional)
    }
}
