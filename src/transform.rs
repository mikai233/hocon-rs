use crate::value::Value;
use ahash::HashMap;
use serde_json::Number;
use std::iter::once;

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Number(value.into())
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Number(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Number::from_f64(value).map_or(Value::Null, Value::Number)
    }
}

impl From<HashMap<String, Value>> for Value {
    fn from(value: HashMap<String, Value>) -> Self {
        Value::Object(value)
    }
}

impl From<(String, Value)> for Value {
    fn from(value: (String, Value)) -> Self {
        Value::Object(HashMap::from_iter(once(value)))
    }
}

impl From<(&str, Value)> for Value {
    fn from(value: (&str, Value)) -> Self {
        let (k, v) = value;
        Value::Object(HashMap::from_iter(once((k.to_string(), v))))
    }
}

impl From<Vec<(String, Value)>> for Value {
    fn from(value: Vec<(String, Value)>) -> Self {
        Value::Object(HashMap::from_iter(value))
    }
}

impl From<Vec<(&str, Value)>> for Value {
    fn from(value: Vec<(&str, Value)>) -> Self {
        Value::Object(value.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::Array(value)
    }
}

impl From<Number> for Value {
    fn from(value: Number) -> Self {
        Value::Number(value)
    }
}