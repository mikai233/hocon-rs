use ahash::{HashMap, HashMapExt};
use itertools::Itertools;
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;
use std::collections::BTreeMap;
use std::collections::hash_map::Entry;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Object(HashMap<String, Value>),
    Array(Vec<Value>),
    Boolean(bool),
    Null,
    String(String),
    Number(Number),
}

impl Value {
    pub fn new_object() -> Value {
        Value::Object(Default::default())
    }

    pub fn with_object<K, I>(values: I) -> Value
    where
        K: Into<String>,
        I: IntoIterator<Item = (K, Value)>,
    {
        let values = values.into_iter().map(|(k, v)| (k.into(), v));
        Value::Object(HashMap::from_iter(values))
    }

    pub fn new_array() -> Value {
        Value::Array(vec![])
    }

    pub fn with_array<I>(values: I) -> Value
    where
        I: IntoIterator<Item = Value>,
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
}

impl Value {
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, Value>> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(boolean) => Some(*boolean),
            _ => None,
        }
    }

    pub fn as_boolean_mut(&mut self) -> Option<&mut bool> {
        match self {
            Value::Boolean(boolean) => Some(boolean),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(string) => Some(string),
            _ => None,
        }
    }

    pub fn as_str_mut(&mut self) -> Option<&mut String> {
        match self {
            Value::String(string) => Some(string),
            _ => None,
        }
    }

    pub fn as_f64(&mut self) -> Option<f64> {
        match self {
            Value::Number(number) => number.as_f64(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(number) => number.as_i64(),
            _ => None,
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
            Value::Number(_) => "Number",
        }
    }

    pub fn into_object(self) -> Option<HashMap<String, Value>> {
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

    /// Get value from the given path and returns `None` if it's invalid.
    ///
    /// A path is considered invalid if:
    /// - It is empty
    /// - Contains leading or trailing `.` or `..` components
    pub fn get_by_path<'a>(&self, paths: impl AsRef<[&'a str]>) -> Option<&Value> {
        let paths = paths.as_ref();
        if paths.is_empty() {
            return None;
        }
        let mut current = self;
        for &path in paths {
            if let Value::Object(obj) = current {
                if let Some(val) = obj.get(path) {
                    current = val;
                } else {
                    return None;
                }
            }
        }
        Some(current)
    }

    pub fn get_by_path_mut<'a>(&mut self, paths: impl AsRef<[&'a str]>) -> Option<&mut Value> {
        let paths = paths.as_ref();
        if paths.is_empty() {
            return None;
        }
        let mut current = self;
        for &path in paths {
            if let Value::Object(obj) = current {
                if let Some(val) = obj.get_mut(path) {
                    current = val;
                } else {
                    return None;
                }
            }
        }
        Some(current)
    }

    /// Merge this `Value` with a fallback `Value`, following HOCON's `withFallback` semantics.
    ///
    /// - If both `self` and `fallback` are `Object`s, they are merged key by key:
    ///   - If a key exists in both objects:
    ///     - If both values are objects, merge them recursively.
    ///     - Otherwise, keep the value from `self` (ignore the fallback).
    ///   - If a key exists only in the fallback, insert it into `self`.
    /// - For all other cases (non-object values), `self` takes precedence
    ///   and the fallback is ignored.
    pub fn with_fallback(self, fallback: Value) -> Value {
        match (self, fallback) {
            // Case 1: Both values are objects -> perform deep merge
            (Value::Object(mut obj), Value::Object(fb_obj)) => {
                for (k, fb_val) in fb_obj {
                    match obj.entry(k) {
                        // If key already exists in `self`
                        Entry::Occupied(mut occupied_entry) => {
                            let existing_val = occupied_entry.get_mut();

                            // If both values are objects -> merge recursively
                            if let (Value::Object(_), Value::Object(_)) = (&existing_val, &fb_val) {
                                // Temporarily move out the existing value to avoid borrow conflicts
                                let mut temp = Value::Null;
                                std::mem::swap(&mut temp, existing_val);

                                // Recursively merge and store back
                                *existing_val = temp.with_fallback(fb_val);
                            }
                            // Otherwise: keep `self`'s value, ignore fallback
                        }

                        // If key is missing in `self` -> insert fallback value
                        Entry::Vacant(vacant_entry) => {
                            vacant_entry.insert(fb_val);
                        }
                    }
                }
                Value::Object(obj)
            }

            // Case 2: Non-object values -> always prefer `self`
            (other, _) => other,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Object(object) => {
                write!(f, "{{")?;
                let last_index = object.len().saturating_sub(1);
                for (index, (k, v)) in object.iter().enumerate() {
                    write!(f, "{} = {}", k, v)?;
                    if index != last_index {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "}}")?;
                Ok(())
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
            Value::Number(number) => {
                write!(f, "{}", number)
            }
        }
    }
}

impl TryFrom<crate::merge::value::Value> for Value {
    type Error = crate::error::Error;

    fn try_from(value: crate::merge::value::Value) -> Result<Self, Self::Error> {
        fn from_object(object: crate::merge::object::Object) -> crate::Result<Value> {
            let inner: BTreeMap<_, _> = object.into();
            let mut object = HashMap::with_capacity(inner.len());
            for (k, v) in inner.into_iter() {
                let v = v.into_inner();
                let v: Value = v.try_into()?;
                object.insert(k, v);
            }
            Ok(Value::Object(object))
        }

        fn from_array(array: crate::merge::array::Array) -> crate::Result<Value> {
            let mut result = Vec::with_capacity(array.len());
            for ele in array.0.into_iter() {
                let v = ele.into_inner();
                let v: Value = v.try_into()?;
                result.push(v);
            }
            Ok(Value::Array(result))
        }
        let value = match value {
            crate::merge::value::Value::Object(object) => from_object(object)?,
            crate::merge::value::Value::Array(array) => from_array(array)?,
            crate::merge::value::Value::Boolean(boolean) => Value::Boolean(boolean),
            crate::merge::value::Value::Null => Value::Null,
            crate::merge::value::Value::String(string) => Value::String(string),
            crate::merge::value::Value::Number(number) => Value::Number(number),
            crate::merge::value::Value::Substitution(_)
            | crate::merge::value::Value::Concat(_)
            | crate::merge::value::Value::AddAssign(_)
            | crate::merge::value::Value::DelayReplacement(_) => {
                return Err(crate::error::Error::SubstitutionNotComplete);
            }
        };
        Ok(value)
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Object(map) => map.serialize(serializer),
            Value::Array(arr) => arr.serialize(serializer),
            Value::Boolean(b) => b.serialize(serializer),
            Value::Null => serializer.serialize_none(),
            Value::String(s) => s.serialize(serializer),
            Value::Number(num) => num.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid HOCON value")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Value::Boolean(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Value::Number(Number::from(v)))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Value::Number(Number::from(v)))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Number::from_f64(v)
                    .map(Value::Number)
                    .ok_or_else(|| Error::custom("invalid f64 value"))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Value::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Value::String(v))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut vec = Vec::new();
                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }
                Ok(Value::Array(vec))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut values = HashMap::new();
                while let Some((k, v)) = map.next_entry()? {
                    values.insert(k, v);
                }
                Ok(Value::Object(values))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}
