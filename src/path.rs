use crate::error::Error;
use derive_more::Constructor;
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Constructor)]
pub struct Path {
    first: String,
    remainder: Option<Box<Path>>,
}

impl Path {
    pub fn with_paths(paths: impl AsRef<str>) -> crate::Result<Option<Self>> {
        let trimmed = paths.as_ref().trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidPathExpression("path is empty"));
        }
        if trimmed.starts_with('.') {
            return Err(Error::InvalidPathExpression("leading period '.' not allowed"));
        }
        if trimmed.ends_with('.') {
            return Err(Error::InvalidPathExpression("trailing period '.' not allowed"));
        }
        if trimmed.contains("..") {
            return Err(Error::InvalidPathExpression("adjacent periods '..' not allowed"));
        }
        let mut dummy = Path::new("".to_string(), None);
        let mut curr = &mut dummy;
        for p in trimmed.split('.') {
            curr.remainder = Some(Path::new(p.to_string(), None).into());
            curr = curr.remainder.as_mut().unwrap();
        }
        Ok(dummy.remainder.map(|p| *p))
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
        while let Some(p) = curr && remove_from_fron > 0 {
            remove_from_fron -= 1;
            curr = p.remainder.as_ref().map(|p| &**p);
        }
        curr
    }

    pub fn pop_front(&self) -> Option<&Path> {
        self.remainder.as_ref().map(|p| &**p)
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut paths = vec![&self.first];
        let mut remainder = &self.remainder;
        while let Some(p) = remainder {
            paths.push(&p.first);
            remainder = &p.remainder;
        }
        write!(f, "{}", paths.iter().join("."))
    }
}