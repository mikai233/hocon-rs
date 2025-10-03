use std::{
    fmt::{Display, Formatter, Write},
    rc::Rc,
};

use derive_more::Constructor;

use crate::path::{Key, Path};

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Constructor)]
pub(crate) struct Substitution {
    pub(crate) path: Rc<Path>,
    pub(crate) optional: bool,
}

impl Substitution {
    pub(crate) fn full_path(&self) -> String {
        self.path.iter().fold(String::new(), |mut acc, next| {
            match &next.first {
                Key::String(s) => {
                    acc.push_str(s);
                }
                Key::Index(i) => {
                    write!(&mut acc, "{i}").unwrap();
                }
            }
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
