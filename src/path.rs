use crate::error::Error;
use derive_more::Constructor;
use itertools::Itertools;
use std::fmt::Display;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Constructor)]
pub struct Path {
    pub first: String,
    pub remainder: Option<Box<Path>>,
}

impl Path {
    pub fn from_str(paths: impl AsRef<str>) -> crate::Result<Self> {
        let trimmed = paths.as_ref().trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidPathExpression("path is empty"));
        }
        if trimmed.starts_with('.') {
            return Err(Error::InvalidPathExpression(
                "leading period '.' not allowed",
            ));
        }
        if trimmed.ends_with('.') {
            return Err(Error::InvalidPathExpression(
                "trailing period '.' not allowed",
            ));
        }
        if trimmed.contains("..") {
            return Err(Error::InvalidPathExpression(
                "adjacent periods '..' not allowed",
            ));
        }
        Self::from_iter(trimmed.split('.'))
    }

    pub fn from_iter<I, V>(paths: I) -> crate::Result<Path>
    where
        I: Iterator<Item = V>,
        V: AsRef<str>,
    {
        let mut dummy = Path::new("".to_string(), None);
        let mut curr = &mut dummy;
        for p in paths {
            let p = p.as_ref();
            curr.remainder = Some(Path::new(p.to_string(), None).into());
            curr = curr.remainder.as_mut().unwrap();
        }
        match dummy.remainder {
            Some(path) => Ok(*path),
            None => Err(crate::error::Error::InvalidPathExpression("path is empty")),
        }
    }

    pub fn len(&self) -> usize {
        let mut len = 1;
        let mut remainder = &self.remainder;
        while let Some(p) = remainder {
            len += 1;
            remainder = &p.remainder;
        }
        len
    }

    pub fn sub_path(&self, mut remove_from_fron: usize) -> Option<&Path> {
        let mut curr = Some(self);
        while let Some(p) = curr
            && remove_from_fron > 0
        {
            remove_from_fron -= 1;
            curr = p.remainder.as_ref().map(|p| &**p);
        }
        curr
    }

    pub fn next(&self) -> Option<&Path> {
        self.remainder.as_ref().map(|p| &**p)
    }

    pub fn push_back(&mut self, path: Path) {
        let tail = self.tail_mut();
        tail.remainder = Some(Box::new(path));
    }

    pub fn tail(&self) -> &Path {
        let mut tail = self;
        while let Some(next) = tail.remainder.as_ref() {
            tail = &*next;
        }
        tail
    }

    pub fn tail_mut(&mut self) -> &mut Path {
        let mut tail = self;
        while tail.remainder.is_some() {
            tail = tail.remainder.as_mut().unwrap();
        }
        tail
    }

    pub fn starts_with0(&self, other: &Path) -> bool {
        let mut left = Some(self);
        let mut right = Some(other);
        loop {
            match (left, right) {
                (Some(l), Some(r)) => {
                    if l.first != r.first {
                        return false;
                    }
                    left = l.remainder.as_ref().map(|p| &**p);
                    right = r.remainder.as_ref().map(|p| &**p);
                }
                (Some(_), None) => return true,
                _ => {
                    return false;
                }
            }
        }
    }

    pub fn starts_with1(&self, other: &[&str]) -> bool {
        if other.is_empty() {
            return false;
        }
        let mut left = Some(self);
        for &p in other {
            match left {
                None => {
                    return false;
                }
                Some(l) => {
                    if p != l.first {
                        return false;
                    }
                    left = l.remainder.as_ref().map(|p| &**p);
                }
            }
        }
        true
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut paths = vec![&self.first];
        let mut remainder = &self.remainder;
        while let Some(p) = remainder {
            paths.push(&p.first);
            remainder = &p.remainder;
        }
        write!(f, "{}", paths.iter().join("."))
    }
}

pub struct Iter<'a> {
    next: Option<&'a Path>,
}

impl Path {
    pub fn iter(&self) -> Iter<'_> {
        Iter { next: Some(self) }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.next {
            self.next = node.remainder.as_deref(); // `Box<Path>` -> `&Path`
            Some(node)
        } else {
            None
        }
    }
}
