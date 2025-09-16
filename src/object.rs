use crate::join_format;
use crate::value::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Object(HashMap<String, Value>);

impl Object {
    pub fn new() -> Self {
        Default::default()
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
        join_format(
            self.iter(),
            f,
            |f| write!(f, ", "),
            |f, (k, v)| write!(f, "{k}: {v}"),
        )
    }
}

impl From<Object> for HashMap<String, Value> {
    fn from(val: Object) -> Self {
        val.0
    }
}

impl FromIterator<(String, Value)> for Object {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}
