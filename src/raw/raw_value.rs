use crate::raw::add_assign::AddAssign;
use crate::raw::concat::Concat;
use crate::raw::include::Inclusion;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::substitution::Substitution;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum RawValue {
    Object(RawObject),
    Array(RawArray),
    Boolean(bool),
    Null,
    String(String),
    UnquotedString(String),
    Float(f64),
    Int(i64),
    Inclusion(Inclusion),
    Substitution(Substitution),
    Concat(Concat),
    AddAssign(AddAssign),
}

impl RawValue {
    pub fn ty(&self) -> &'static str {
        match self {
            RawValue::Object(_) => "object",
            RawValue::Array(_) => "array",
            RawValue::Boolean(_) => "boolean",
            RawValue::Null => "null",
            RawValue::String(_) => "string",
            RawValue::UnquotedString(_) => "unquoted_string",
            RawValue::Float(_) => "float",
            RawValue::Int(_) => "int",
            RawValue::Inclusion(_) => "inclusion",
            RawValue::Substitution(_) => "substitution",
            RawValue::Concat(_) => "concat",
            RawValue::AddAssign(_) => "add_assign",
        }
    }
}

impl Display for RawValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawValue::Object(object) => write!(f, "Object({})", object),
            RawValue::Array(array) => write!(f, "Array({})", array),
            RawValue::Boolean(boolean) => write!(f, "Boolean({})", boolean),
            RawValue::Null => write!(f, "Null"),
            RawValue::String(string) => write!(f, "QuotedString({})", string),
            RawValue::UnquotedString(string) => write!(f, "UnquotedString({})", string),
            RawValue::Float(float) => write!(f, "Float({})", float),
            RawValue::Int(int) => write!(f, "Int({})", int),
            RawValue::Inclusion(inclusion) => write!(f, "Inclusion({})", inclusion),
            RawValue::Substitution(substitution) => write!(f, "Substitution({})", substitution),
            RawValue::Concat(concat) => write!(f, "Concat({})", concat),
            RawValue::AddAssign(add_assign) => write!(f, "AddAssign({})", add_assign),
        }
    }
}