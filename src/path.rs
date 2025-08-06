use crate::error::Error;
use std::ops::Deref;

#[derive(Debug)]
pub struct Path(Vec<String>);

impl Path {
    pub fn new(path: impl AsRef<str>) -> crate::Result<Self> {
        let trimmed = path.as_ref().trim();
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
        Ok(Self(trimmed.split('.').map(ToString::to_string).collect()))
    }

    pub fn join(&mut self, path: impl AsRef<str>)  {

    }
}

impl Deref for Path {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}