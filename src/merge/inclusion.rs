use derive_more::Constructor;

use crate::{merge::object::Object, raw::include::Location};

#[derive(Debug, PartialEq, Clone, Constructor)]
pub(crate) struct Inclusion {
    pub(crate) path: String,
    pub(crate) required: bool,
    pub(crate) location: Option<Location>,
    pub(crate) val: Option<Box<Object>>,
}

impl TryFrom<crate::raw::include::Inclusion> for Inclusion {
    type Error = crate::error::Error;

    fn try_from(value: crate::raw::include::Inclusion) -> Result<Self, Self::Error> {
        let crate::raw::include::Inclusion {
            path,
            required,
            location,
            val,
        } = value;
        let val = match val {
            Some(val) => {
                let val: Object = (*val).try_into()?;
                Some(Box::new(val))
            }
            None => None,
        };
        Ok(Self::new(path, required, location, val))
    }
}
