use std::fmt::Display;

use derive_more::Constructor;

use crate::{
    join,
    path::{Key, Path},
};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Constructor)]
pub(crate) struct RefPath<'a> {
    pub first: RefKey<'a>,
    pub remainder: Option<Box<RefPath<'a>>>,
}

impl<'a> RefPath<'a> {
    pub fn from_slice(paths: &'a [&'a str]) -> crate::Result<RefPath<'a>> {
        let mut dummy = RefPath::new(RefKey::Str(""), None);
        let mut curr = &mut dummy;
        for p in paths {
            curr.remainder = Some(RefPath::new(RefKey::Str(p), None).into());
            curr = curr.remainder.as_mut().unwrap();
        }
        match dummy.remainder {
            Some(path) => Ok(*path),
            None => Err(crate::error::Error::InvalidPathExpression("path is empty")),
        }
    }

    pub fn next(&self) -> Option<&RefPath<'a>> {
        self.remainder.as_deref()
    }

    pub fn join(&self, path: RefPath<'a>) -> RefPath<'a> {
        let mut cloned = self.clone();
        let tail = cloned.tail_mut();
        tail.remainder = Some(Box::new(path));
        cloned
    }

    pub fn tail_mut(&mut self) -> &mut RefPath<'a> {
        let mut tail = self;
        while tail.remainder.is_some() {
            tail = tail.remainder.as_mut().unwrap();
        }
        tail
    }

    pub fn from(path: &Path) -> RefPath<'_> {
        let mut dummy = RefPath::new(RefKey::Str(""), None);
        let mut tail = &mut dummy;
        for ele in path.iter() {
            let p = RefPath::new(RefKey::from_owned(&ele.first), None);
            tail.remainder = Some(Box::new(p));
            tail = tail.remainder.as_mut().unwrap();
        }
        *dummy.remainder.unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RefKey<'a> {
    Str(&'a str),
    Index(usize),
}

impl<'a> RefKey<'a> {
    pub(crate) fn to_owned(&self) -> Key {
        match self {
            RefKey::Str(s) => Key::String(s.to_string()),
            RefKey::Index(i) => Key::Index(*i),
        }
    }

    pub(crate) fn from_owned(key: &Key) -> RefKey<'_> {
        match key {
            Key::String(s) => RefKey::Str(s),
            Key::Index(i) => RefKey::Index(*i),
        }
    }
}

impl<'a> Display for RefKey<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefKey::Str(s) => write!(f, "{s}"),
            RefKey::Index(i) => write!(f, "{i}"),
        }
    }
}

impl<'a> PartialEq<Key> for RefKey<'a> {
    fn eq(&self, other: &Key) -> bool {
        match (self, other) {
            (RefKey::Str(a), Key::String(b)) => a == b,
            (RefKey::Str(_), Key::Index(_)) | (RefKey::Index(_), Key::String(_)) => false,
            (RefKey::Index(a), Key::Index(b)) => a == b,
        }
    }
}

impl<'a> PartialEq<RefKey<'a>> for Key {
    fn eq(&self, other: &RefKey<'a>) -> bool {
        match (self, other) {
            (Key::String(a), RefKey::Str(b)) => a == b,
            (Key::String(_), RefKey::Index(_)) | (Key::Index(_), RefKey::Str(_)) => false,
            (Key::Index(a), RefKey::Index(b)) => a == b,
        }
    }
}

impl From<RefPath<'_>> for Path {
    fn from(val: RefPath<'_>) -> Self {
        let mut dummy = Path::new(Key::String("".to_string()), None);
        let mut tail = &mut dummy;
        let mut current = Some(&val);
        while let Some(p) = current {
            tail.remainder = Some(Path::new(p.first.to_owned(), None).into());
            tail = tail.remainder.as_mut().unwrap();
            current = p.next();
        }
        *dummy.remainder.unwrap()
    }
}

impl Display for RefPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut paths = vec![&self.first];
        let mut remainder = &self.remainder;
        while let Some(p) = remainder {
            paths.push(&p.first);
            remainder = &p.remainder;
        }
        join(paths.iter(), ".", f)
    }
}

macro_rules! impl_path_eq {
    ($path1:ty, $path2:ty) => {
        impl PartialEq<$path1> for $path2 {
            fn eq(&self, other: &$path1) -> bool {
                let mut left = Some(self);
                let mut right = Some(other);
                while let (Some(l), Some(r)) = (left, right) {
                    if l.first != r.first {
                        return false;
                    }
                    left = l.next();
                    right = r.next();
                }
                left.is_none() && right.is_none()
            }
        }
    };
}

impl_path_eq!(crate::path::Path, RefPath<'_>);

impl_path_eq!(RefPath<'_>, crate::path::Path);
