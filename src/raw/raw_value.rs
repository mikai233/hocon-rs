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

pub const RAW_OBJECT_TYPE: &'static str = "object";
pub const RAW_ARRAY_TYPE: &'static str = "array";
pub const RAW_BOOLEAN_TYPE: &'static str = "boolean";
pub const RAW_NULL_TYPE: &'static str = "null";
pub const RAW_QUOTED_STRING_TYPE: &'static str = "quoted_string";
pub const RAW_UNQUOTED_STRING_TYPE: &'static str = "unquoted_string";
pub const RAW_MULTILINE_STRING_TYPE: &'static str = "multiline_string";
pub const RAW_CONCAT_STRING_TYPE: &'static str = "concat_string";
pub const RAW_NUMBER_TYPE: &'static str = "number";
pub const RAW_SUBSTITUTION_TYPE: &'static str = "substitution";
pub const RAW_CONCAT_TYPE: &'static str = "concat";
pub const RAW_ADD_ASSIGN_TYPE: &'static str = "add_assign";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RawValue {
    Object(RawObject),
    Array(RawArray),
    Boolean(bool),
    Null,
    String(RawString),
    Number(Number),
    Substitution(Substitution),
    Concat(Concat),
    AddAssign(AddAssign),
}

impl RawValue {
    pub fn ty(&self) -> &'static str {
        match self {
            RawValue::Object(_) => RAW_OBJECT_TYPE,
            RawValue::Array(_) => RAW_ARRAY_TYPE,
            RawValue::Boolean(_) => RAW_BOOLEAN_TYPE,
            RawValue::Null => RAW_NULL_TYPE,
            RawValue::String(s) => s.ty(),
            RawValue::Number(_) => RAW_NUMBER_TYPE,
            RawValue::Substitution(_) => RAW_SUBSTITUTION_TYPE,
            RawValue::Concat(_) => RAW_CONCAT_TYPE,
            RawValue::AddAssign(_) => RAW_ADD_ASSIGN_TYPE,
        }
    }

    pub fn is_simple_value(&self) -> bool {
        matches!(
            self,
            RawValue::Boolean(_) | RawValue::Null | RawValue::String(_) | RawValue::Number(_)
        ) || matches!(self, RawValue::AddAssign(r) if r.is_simple_value())
    }

    pub fn inclusion(inclusion: Inclusion) -> RawValue {
        let field = ObjectField::inclusion(inclusion);
        RawValue::Object(RawObject::new(vec![field]))
    }

    pub fn key_value<I>(fields: I) -> RawValue
    where
        I: IntoIterator<Item = (RawString, RawValue)>,
    {
        RawValue::Object(RawObject::key_value(fields))
    }

    pub fn object(object: impl Into<RawObject>) -> RawValue {
        RawValue::Object(object.into())
    }

    pub fn array<I>(iter: I) -> RawValue
    where
        I: IntoIterator<Item = RawValue>,
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
        I: IntoIterator<Item = (RawString, Option<String>)>,
        S: Into<String>,
    {
        RawValue::String(RawString::concat(iter))
    }

    pub fn number(n: impl Into<Number>) -> RawValue {
        RawValue::Number(n.into())
    }

    pub fn substitution(s: Substitution) -> RawValue {
        RawValue::Substitution(s)
    }

    pub fn concat<I>(iter: I) -> RawValue
    where
        I: IntoIterator<Item = RawValue>,
    {
        RawValue::Concat(Concat::new(iter.into_iter().collect_vec()).unwrap())
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
            RawValue::Substitution(substitution) => write!(f, "{}", substitution),
            RawValue::Concat(concat) => write!(f, "{}", concat),
            RawValue::AddAssign(add_assign) => write!(f, "{}", add_assign),
        }
    }
}

impl TryInto<RawArray> for RawValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<RawArray, Self::Error> {
        match self {
            RawValue::Array(a) => Ok(a),
            other => Err(crate::error::Error::InvalidConversion {
                from: other.ty(),
                to: RAW_ARRAY_TYPE,
            }),
        }
    }
}

impl TryInto<RawObject> for RawValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<RawObject, Self::Error> {
        match self {
            RawValue::Object(o) => Ok(o),
            other => Err(crate::error::Error::InvalidConversion {
                from: other.ty(),
                to: RAW_OBJECT_TYPE,
            }),
        }
    }
}

impl Into<RawValue> for serde_json::Value {
    fn into(self) -> RawValue {
        match self {
            serde_json::Value::Null => RawValue::Null,
            serde_json::Value::Bool(boolean) => RawValue::Boolean(boolean),
            serde_json::Value::Number(number) => RawValue::Number(number),
            serde_json::Value::String(string) => RawValue::String(string.into()),
            serde_json::Value::Array(values) => RawValue::array(values.into_iter().map(Into::into)),
            serde_json::Value::Object(map) => {
                let fields = map
                    .into_iter()
                    .map(|(key, value)| ObjectField::key_value(key, value))
                    .collect();
                let raw_object = RawObject::new(fields);
                RawValue::Object(raw_object)
            }
        }
    }
}
