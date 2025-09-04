use std::fmt::Display;

use derive_more::Constructor;

use crate::join;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Constructor)]
pub(crate) struct RefPath<'a> {
    pub first: &'a str,
    pub remainder: Option<Box<RefPath<'a>>>,
}

impl<'a> RefPath<'a> {
    pub fn from_slice(paths: &'a [&'a str]) -> crate::Result<RefPath<'a>> {
        let mut dummy = RefPath::new("", None);
        let mut curr = &mut dummy;
        for p in paths {
            curr.remainder = Some(RefPath::new(p, None).into());
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

    pub fn from(path: &crate::path::Path) -> RefPath<'_> {
        let mut dummy = RefPath::new("", None);
        let mut tail = &mut dummy;
        for ele in path.iter() {
            let p = RefPath::new(&ele.first, None);
            tail.remainder = Some(Box::new(p));
            tail = tail.remainder.as_mut().unwrap();
        }
        *dummy.remainder.unwrap()
    }
}

impl From<RefPath<'_>> for crate::path::Path {
    fn from(val: RefPath<'_>) -> Self {
        let mut dummy = crate::path::Path::new("".to_string(), None);
        let mut tail = &mut dummy;
        let mut current = Some(&val);
        while let Some(p) = current {
            tail.remainder = Some(crate::path::Path::new(p.first.to_string(), None).into());
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
