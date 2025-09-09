use derive_more::Constructor;

use crate::raw::raw_object::RawObject;
use std::{fmt::Display, rc::Rc};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Constructor)]
pub struct Inclusion {
    pub path: Rc<String>,
    pub required: bool,
    pub location: Option<Location>,
    pub val: Option<Box<RawObject>>,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Location {
    File,
    #[cfg(feature = "urls_includes")]
    Url,
    Classpath,
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Location::File => write!(f, "file"),
            #[cfg(feature = "urls_includes")]
            Location::Url => write!(f, "url"),
            Location::Classpath => write!(f, "classpath"),
        }
    }
}

impl Display for Inclusion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "include ")?;
        if self.required {
            write!(f, "required(")?;
        }
        match self.location {
            None => {
                write!(f, "\"{}\"", self.path)?;
            }
            Some(location) => {
                write!(f, "{}(", location)?;
                write!(f, "\"{}\"", self.path)?;
                write!(f, ")")?;
            }
        }
        if self.required {
            write!(f, ")")?;
        }
        Ok(())
    }
}
