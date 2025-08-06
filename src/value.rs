use crate::object::Object;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Object(Object),
    Array(Vec<Value>),
    Boolean(bool),
    Null,
    String(String),
    Float(f64),
    Int(i64),
}

impl Value {
    pub fn new_object() -> Value {
        Value::Object(Object::new())
    }

    pub fn with_object<K, I>(values: I) -> Value
    where
        K: Into<String>,
        I: IntoIterator<Item=(K, Value)>,
    {
        let values = values.into_iter().map(|(k, v)| (k.into(), v));
        Value::Object(Object::with_kvs(values))
    }

    pub fn new_array() -> Value {
        Value::Array(vec![])
    }

    pub fn with_array<I>(values: I) -> Value
    where
        I: IntoIterator<Item=Value>,
    {
        Value::Array(values.into_iter().collect())
    }

    pub fn new_boolean(boolean: bool) -> Value {
        Value::Boolean(boolean)
    }

    pub fn new_null() -> Value {
        Value::Null
    }

    pub fn new_string(string: impl Into<String>) -> Value {
        Value::String(string.into())
    }

    pub fn new_float(float: f64) -> Value {
        Value::Float(float)
    }

    pub fn new_int(int: i64) -> Value {
        Value::Int(int)
    }
}

impl Value {
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(object) => Some(object),
            _ => None
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Object> {
        match self {
            Value::Object(object) => Some(object),
            _ => None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(boolean) => Some(*boolean),
            _ => None
        }
    }

    pub fn as_boolean_mut(&mut self) -> Option<&mut bool> {
        match self {
            Value::Boolean(boolean) => Some(boolean),
            _ => None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(string) => Some(string),
            _ => None
        }
    }

    pub fn as_str_mut(&mut self) -> Option<&mut String> {
        match self {
            Value::String(string) => Some(string),
            _ => None
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(float) => Some(*float),
            _ => None
        }
    }

    pub fn as_float_mut(&mut self) -> Option<&mut f64> {
        match self {
            Value::Float(float) => Some(float),
            _ => None
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(int) => Some(*int),
            _ => None
        }
    }

    pub fn as_int_mut(&mut self) -> Option<i64> {
        match self {
            Value::Int(int) => Some(*int),
            _ => None
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn ty(&self) -> &'static str {
        match self {
            Value::Object(_) => "Object",
            Value::Array(_) => "Array",
            Value::Boolean(_) => "Boolean",
            Value::Null => "Null",
            Value::String(_) => "String",
            Value::Float(_) => "Float",
            Value::Int(_) => "Int"
        }
    }

    pub fn into_object(self) -> Option<Object> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn into_array(self) -> Option<Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn into_boolean(self) -> Option<bool> {
        match self {
            Value::Boolean(boolean) => Some(boolean),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<String> {
        match self {
            Value::String(string) => Some(string),
            _ => None,
        }
    }

    pub fn into_float(self) -> Option<f64> {
        match self {
            Value::Float(float) => Some(float),
            _ => None,
        }
    }

    pub fn into_int(self) -> Option<i64> {
        match self {
            Value::Int(int) => Some(int),
            _ => None,
        }
    }
}

impl Value {
    fn serialize<T: Serialize>(&self) -> T {
        todo!()
    }

    fn deserialize<'a, T: Deserialize<'a>>(&'a self) -> T {
        todo!()
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Object(object) => {
                write!(f, "{}", object)
            }
            Value::Array(array) => {
                write!(f, "[{}]", array.iter().join(", "))
            }
            Value::Boolean(boolean) => {
                write!(f, "{}", boolean)
            }
            Value::Null => {
                write!(f, "null")
            }
            Value::String(string) => {
                write!(f, "{}", string)
            }
            Value::Float(float) => {
                write!(f, "{}", float)
            }
            Value::Int(int) => {
                write!(f, "{}", int)
            }
        }
    }
}