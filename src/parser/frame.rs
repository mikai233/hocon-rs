use crate::raw::{field::ObjectField, raw_string::RawString, raw_value::RawValue};

#[derive(Debug, Default)]
pub(crate) struct Value {
    pub(crate) values: Vec<RawValue>,
    pub(crate) spaces: Vec<Option<String>>,
    pub(crate) pre_space: Option<String>,
}

impl Value {
    pub(crate) fn push_value(&mut self, value: RawValue) {
        if !self.values.is_empty() {
            self.spaces.push(self.pre_space.take());
        }
        self.values.push(value);
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Separator {
    Assign,
    AddAssign,
}

#[derive(Debug, Default)]
pub(crate) struct Entry {
    pub(crate) key: Option<RawString>,
    pub(crate) separator: Option<Separator>,
    pub(crate) value: Option<Value>,
}

#[derive(Debug)]
pub(crate) enum Frame {
    Object {
        entries: Vec<ObjectField>,
        next_entry: Option<Entry>,
    },
    Array {
        elements: Vec<RawValue>,
        next_element: Option<Value>,
    },
}

impl Frame {
    pub(crate) fn ty(&self) -> &str {
        match &self {
            Frame::Object { .. } => "Frame::Object",
            Frame::Array { .. } => "Frame::Array",
        }
    }
}
