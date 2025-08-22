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

impl Into<Value> for serde_json::Value {
    fn into(self) -> Value {
        match self {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(boolean) => Value::Boolean(boolean),
            serde_json::Value::Number(number) => Value::Number(number),
            serde_json::Value::String(string) => Value::String(string),
            serde_json::Value::Array(array) => Value::with_array(array.into_iter().map(Into::into)),
            serde_json::Value::Object(object) => {
                Value::with_object(object.into_iter().map(|(key, value)| (key, value.into())))
            }
        }
    }
}

impl Into<serde_json::Value> for Value {
    fn into(self) -> serde_json::Value {
        match self {
            Value::Object(object) => {
                let map = serde_json::Map::from_iter(
                    object.into_iter().map(|(key, value)| (key, value.into())),
                );
                serde_json::Value::Object(map)
            }
            Value::Array(array) => {
                serde_json::Value::Array(array.into_iter().map(Into::into).collect())
            }
            Value::Boolean(boolean) => serde_json::Value::Bool(boolean),
            Value::Null => serde_json::Value::Null,
            Value::String(string) => serde_json::Value::String(string),
            Value::Number(number) => serde_json::Value::Number(number),
        }
    }
}
