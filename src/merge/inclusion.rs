use derive_more::Constructor;

use crate::{merge::object::Object, raw::include::Location};

#[derive(Debug, PartialEq, Clone, Constructor)]
pub(crate) struct Inclusion {
    pub(crate) path: String,
    pub(crate) required: bool,
    pub(crate) location: Option<Location>,
    pub(crate) val: Option<Box<Object>>,
}

impl From<crate::raw::include::Inclusion> for Inclusion {
    fn from(value: crate::raw::include::Inclusion) -> Self {
        let crate::raw::include::Inclusion {
            path,
            required,
            location,
            val,
        } = value;
        let val: Option<Box<Object>> = val.map(|v| (*v).into()).map(|v| Box::new(v));
        Self::new(path, required, location, val)
    }
}
