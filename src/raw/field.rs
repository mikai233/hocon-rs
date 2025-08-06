use crate::raw::include::Inclusion;
use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectField {
    Inclusion(Inclusion),
    KeyValue(String, RawValue),
}

impl Display for ObjectField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectField::Inclusion(v) => write!(f, "{}", v),
            ObjectField::KeyValue(k, v) => write!(f, "{}: {}", k, v),
        }
    }
}