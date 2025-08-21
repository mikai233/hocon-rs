use derive_more::Constructor;

use crate::{
    merge::{object::Object, path::RefPath},
    raw::include::Location,
};

#[derive(Debug, PartialEq, Clone, Constructor)]
pub(crate) struct Inclusion {
    pub(crate) path: String,
    pub(crate) required: bool,
    pub(crate) location: Option<Location>,
    pub(crate) val: Option<Box<Object>>,
}

impl Inclusion {
    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::include::Inclusion,
    ) -> crate::Result<Self> {
        let crate::raw::include::Inclusion {
            path,
            required,
            location,
            val,
        } = raw;
        let val = match val {
            Some(val) => {
                let val = Object::from_raw(parent, *val)?;
                Some(Box::new(val))
            }
            None => None,
        };
        Ok(Self::new(path, required, location, val))
    }
}
