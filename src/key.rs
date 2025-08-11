use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
pub struct Key(Vec<String>);

impl Key {
    pub fn new<I, V>(path: I) -> Self
    where
        I: IntoIterator<Item=V>,
        V: Into<String>,
    {
        Key(path.into_iter().map(|k| k.into()).collect())
    }

    pub fn push<P>(&mut self, path: P)
    where
        P: Into<String>,
    {
        self.0.push(path.into());
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("."))
    }
}

impl Into<Key> for &str {
    fn into(self) -> Key {
        Key::new(self.split('.'))
    }
}

impl Into<Key> for String {
    fn into(self) -> Key {
        Key::new(self.split('.'))
    }
}