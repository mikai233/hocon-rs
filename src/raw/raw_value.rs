use crate::Result;
use crate::raw::add_assign::AddAssign;
use crate::raw::concat::Concat;
use crate::raw::field::ObjectField;
use crate::raw::include::Inclusion;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::substitution::Substitution;
use serde_json::Number;
use std::fmt::{Display, Formatter};

pub const RAW_OBJECT_TYPE: &str = "object";
pub const RAW_ARRAY_TYPE: &str = "array";
pub const RAW_BOOLEAN_TYPE: &str = "boolean";
pub const RAW_NULL_TYPE: &str = "null";
pub const RAW_QUOTED_STRING_TYPE: &str = "quoted_string";
pub const RAW_UNQUOTED_STRING_TYPE: &str = "unquoted_string";
pub const RAW_MULTILINE_STRING_TYPE: &str = "multiline_string";
pub const RAW_CONCAT_STRING_TYPE: &str = "concat_string";
pub const RAW_NUMBER_TYPE: &str = "number";
pub const RAW_SUBSTITUTION_TYPE: &str = "substitution";
pub const RAW_CONCAT_TYPE: &str = "concat";
pub const RAW_ADD_ASSIGN_TYPE: &str = "add_assign";

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

    pub fn object(values: Vec<(RawString, RawValue)>) -> RawValue {
        let fields = values
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        RawValue::Object(RawObject::new(fields))
    }

    pub fn array(values: Vec<RawValue>) -> RawValue {
        RawValue::Array(RawArray::new(values))
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

    pub fn path_expression(paths: Vec<RawString>) -> RawValue {
        RawValue::String(RawString::path_expression(paths))
    }

    pub fn number(n: impl Into<Number>) -> RawValue {
        RawValue::Number(n.into())
    }

    pub fn substitution(s: Substitution) -> RawValue {
        RawValue::Substitution(s)
    }

    pub fn concat(values: Vec<RawValue>, spaces: Vec<Option<String>>) -> Result<RawValue> {
        Ok(RawValue::Concat(Concat::new(values, spaces)?))
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

    fn try_into(self) -> Result<RawArray> {
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

    fn try_into(self) -> Result<RawObject> {
        match self {
            RawValue::Object(o) => Ok(o),
            other => Err(crate::error::Error::InvalidConversion {
                from: other.ty(),
                to: RAW_OBJECT_TYPE,
            }),
        }
    }
}

impl From<serde_json::Value> for RawValue {
    fn from(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::Null => RawValue::Null,
            serde_json::Value::Bool(boolean) => RawValue::Boolean(boolean),
            serde_json::Value::Number(number) => RawValue::Number(number),
            serde_json::Value::String(string) => RawValue::String(string.into()),
            serde_json::Value::Array(values) => {
                RawValue::array(values.into_iter().map(Into::into).collect())
            }
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::raw::raw_value::RawValue;

    #[test]
    fn test_from_json() {
        let _: RawValue = json!({
            "a" : null,
            "b" : {"c" : [1,2,3.001], "mikai":233},
            "c":true,
            "d":false,
            "e":"world hello"
        })
        .into();
    }
}
