use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq, Hash, Clone, derive_more::Constructor)]
pub struct Substitution {
    pub path: String,
    pub optional: bool,
}

impl Display for Substitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{path: {}, optional: {}", self.path, self.optional)
    }
}