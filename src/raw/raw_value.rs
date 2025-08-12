use crate::raw::add_assign::AddAssign;
use crate::raw::concat::Concat;
use crate::raw::field::ObjectField;
use crate::raw::include::Inclusion;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::substitution::Substitution;
use itertools::Itertools;
use serde_json::Number;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum RawValue {
    Object(RawObject),
    Array(RawArray),
    Boolean(bool),
    Null,
    String(RawString),
    Number(Number),
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
            RawValue::String(s) => s.ty(),
            RawValue::Number(_) => "number",
            RawValue::Inclusion(_) => "inclusion",
            RawValue::Substitution(_) => "substitution",
            RawValue::Concat(_) => "concat",
            RawValue::AddAssign(_) => "add_assign",
        }
    }

    pub fn object_i(inclusion: Inclusion) -> RawValue {
        let field = ObjectField::Inclusion(inclusion);
        RawValue::Object(RawObject::new(vec![field]))
    }

    pub fn object_kv<I>(iter: I) -> RawValue
    where
        I: IntoIterator<Item=(RawString, RawValue)>,
    {
        RawValue::Object(RawObject::kv(iter))
    }

    pub fn array<I>(iter: I) -> RawValue
    where
        I: IntoIterator<Item=RawValue>,
    {
        RawValue::Array(RawArray::new(iter.into_iter().collect()))
    }

    pub fn boolean(b: bool) -> RawValue {
        RawValue::Boolean(b)
    }

    pub fn null() -> RawValue {
        RawValue::Null
    }

    pub fn quoted_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::quoted(s))
    }

    pub fn unquoted_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::unquoted(s))
    }

    pub fn multiline_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::multiline(s))
    }

    pub fn concat_string<I, S>(iter: I) -> RawValue
    where
        I: IntoIterator<Item=(RawString, Option<String>)>,
        S: Into<String>,
    {
        RawValue::String(RawString::concat(iter))
    }

    pub fn number(n: impl Into<Number>) -> RawValue {
        RawValue::Number(n.into())
    }

    pub fn inclusion(incl: Inclusion) -> RawValue {
        RawValue::Inclusion(incl)
    }

    pub fn substitution(s: Substitution) -> RawValue {
        RawValue::Substitution(s)
    }

    pub fn concat<I>(iter: I) -> RawValue
    where
        I: IntoIterator<Item=RawValue>,
    {
        RawValue::Concat(Concat::new(iter.into_iter().collect_vec()))
    }

    pub fn add_assign(v: RawValue) -> RawValue {
        RawValue::AddAssign(AddAssign::new(v.into()))
    }
}

impl Display for RawValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawValue::Object(object) => write!(f, "{}", object),
            RawValue::Array(array) => write!(f, "{}", array),
            RawValue::Boolean(boolean) => write!(f, "{}", boolean),
            RawValue::Null => write!(f, "null"),
            RawValue::String(string) => write!(f, "{}", string),
            RawValue::Number(number) => write!(f, "{}", number),
            RawValue::Inclusion(inclusion) => write!(f, "{}", inclusion),
            RawValue::Substitution(substitution) => write!(f, "{}", substitution),
            RawValue::Concat(concat) => write!(f, "{}", concat),
            RawValue::AddAssign(add_assign) => write!(f, "{}", add_assign),
        }
    }
}