use crate::value::Value;
use ahash::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Object(HashMap<String, Value>);

impl Object {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        Self(HashMap::from_iter(iter))
    }
}

impl Deref for Object {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.iter();
        match iter.next() {
            Some((k, v)) => {
                write!(f, "{k}: {v}")?;
                for (k, v) in iter {
                    write!(f, ", ")?;
                    write!(f, "{k}: {v}")?;
                }
            }
            None => {}
        }
        Ok(())
    }
}

impl Into<HashMap<String, Value>> for Object {
    fn into(self) -> HashMap<String, Value> {
        self.0
    }
}
