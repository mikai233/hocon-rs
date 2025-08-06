use crate::raw::raw_object::RawObject;
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Clone, derive_more::Constructor)]
pub struct Inclusion {
    pub depth: usize,
    pub path: String,
    pub required: bool,
    pub location: Option<Location>,
    pub val: Option<Box<RawObject>>,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Location {
    File,
    Url,
    Classpath,
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Location::File => write!(f, "file"),
            Location::Url => write!(f, "url"),
            Location::Classpath => write!(f, "classpath"),
        }
    }
}

impl Display for Inclusion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        write!(f, "path: {}", self.path)?;
        write!(f, "required: {};", self.required)?;
        match self.location {
            None => write!(f, "location: None")?,
            Some(location) => write!(f, "{}", location)?,
        }
        write!(f, "}}")?;
        Ok(())
    }
}