use bigdecimal::BigDecimal;
use num_bigint::{BigUint, ToBigInt};
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use std::time::Duration;

use crate::{join, join_format};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Object(HashMap<String, Value>),
    Array(Vec<Value>),
    Boolean(bool),
    Null,
    String(String),
    Number(Number),
}

impl Value {
    pub fn object(obj: HashMap<String, Value>) -> Value {
        Value::Object(obj)
    }

    pub fn object_from_iter<I>(iter: I) -> Value
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        Value::Object(HashMap::from_iter(iter))
    }

    pub fn array(values: Vec<Value>) -> Value {
        Value::Array(values)
    }

    pub fn array_from_iter<I>(iter: I) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        Value::Array(iter.into_iter().collect())
    }

    pub fn boolean(boolean: bool) -> Value {
        Value::Boolean(boolean)
    }

    pub fn null() -> Value {
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

    /// Attempts to interpret the current [`Value`] as an array by applying
    /// HOCON's "numerically-indexed object to array" conversion rule.
    ///
    /// # Behavior
    ///
    /// - If the value is already an array (`Value::Array`), this simply
    ///   returns a reference to its elements as a `Vec<&Value>`.
    ///
    /// - If the value is an object (`Value::Object`) whose keys are strings
    ///   representing integers (e.g. `"0"`, `"1"`, `"2"`), it is converted
    ///   into an array:
    ///   - Keys are filtered to include only those that can be parsed as `usize`.
    ///   - The key–value pairs are sorted by their numeric key.
    ///   - The values are collected into a `Vec<&Value>` in ascending key order.
    ///
    /// - For any other kind of value, the function returns `None`.
    ///
    /// # Example
    ///
    /// ```hocon
    /// {
    ///   "0": "first",
    ///   "2": "third",
    ///   "1": "second"
    /// }
    /// ```
    ///
    /// Will be interpreted as:
    ///
    /// ```hocon
    /// [ "first", "second", "third" ]
    /// ```
    pub fn as_array_numerically(&self) -> Option<Vec<&Value>> {
        match self {
            // Already an array → just return the elements.
            Value::Array(array) => Some(array.iter().collect()),

            // If it's an object, try to convert it to an array using numeric keys.
            Value::Object(object) => {
                // Keep only entries whose keys can be parsed as integers.
                let mut object_array = object
                    .iter()
                    .filter(|(key, _)| key.parse::<usize>().is_ok())
                    .collect::<Vec<_>>();

                // Sort by numeric key so the order is consistent with array semantics.
                object_array.sort_by(|a, b| a.0.cmp(b.0));

                // Extract values in order, discarding the string keys.
                let array = object_array
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>();

                Some(array)
            }

            // Not an array and not an object → cannot convert.
            _ => None,
        }
    }

    /// Attempts to interpret the current [`Value`] as a boolean, following
    /// HOCON's relaxed truthy/falsey rules.
    ///
    /// # Behavior
    ///
    /// - If the value is a `Value::Boolean`, returns the inner `bool`.
    ///
    /// - If the value is a `Value::String`, accepts several textual
    ///   representations:
    ///   - `"true"`, `"on"`, `"yes"` → `Some(true)`
    ///   - `"false"`, `"off"`, `"no"` → `Some(false)`
    ///
    /// - For all other values (numbers, arrays, objects, or strings that
    ///   don't match the above), returns `None`.
    ///
    /// # Notes
    /// - The matching is **case-sensitive** (`"True"` will not be recognized).
    /// - This conversion is specific to HOCON and goes beyond JSON’s strict
    ///   boolean representation.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            // Direct boolean value
            Value::Boolean(boolean) => Some(*boolean),

            // String representations of truthy values
            Value::String(boolean) if boolean == "true" || boolean == "on" || boolean == "yes" => {
                Some(true)
            }

            // String representations of falsey values
            Value::String(boolean) if boolean == "false" || boolean == "off" || boolean == "no" => {
                Some(false)
            }

            // Everything else → not interpretable as boolean
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(string) => Some(string),
            _ => None,
        }
    }

    pub fn as_f64(&mut self) -> Option<f64> {
        match self {
            Value::Number(number) => number.as_f64(),
            Value::String(number) => number.parse().ok(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(number) => number.as_i64(),
            Value::String(number) => number.parse().ok(),
            _ => None,
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Value::Number(number) => number.as_i128(),
            Value::String(number) => number.parse().ok(),
            _ => None,
        }
    }

    pub fn as_u128(&self) -> Option<u128> {
        match self {
            Value::Number(number) => number.as_u128(),
            Value::String(number) => number.parse().ok(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(number) => number.as_u64(),
            Value::String(number) => number.parse().ok(),
            _ => None,
        }
    }

    /// Checks whether the current [`Value`] represents `null` in HOCON.
    ///
    /// # Behavior
    ///
    /// - Returns `true` if the value is explicitly `Value::Null`.
    /// - Returns `true` if the value is a `Value::String` equal to `"null"`.
    /// - Otherwise, returns `false`.
    ///
    /// # Notes
    /// - The check for `"null"` is **case-sensitive**. `"Null"` or `"NULL"`
    ///   will not be considered null.
    /// - This deviates from strict JSON, where only a literal `null` is valid.
    ///   HOCON allows the string `"null"` to be treated as a null value.
    pub fn is_null(&self) -> bool {
        match self {
            Value::Null => true,
            Value::String(s) if s == "null" => true,
            _ => false,
        }
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

    pub fn into_number(self) -> Option<serde_json::Number> {
        match self {
            Value::Number(number) => Some(number),
            _ => None,
        }
    }

    /// Retrieves a value from a nested `Value::Object` by following a HOCON-style path.
    ///
    /// # Arguments
    ///
    /// * `paths` - A sequence of keys representing the path to the desired value.
    ///   The path should already be split by `.` (dot).
    ///
    /// # Returns
    ///
    /// * `Some(&Value)` if the full path exists in the object tree.
    /// * `None` if any key in the path does not exist or if a non-object value is encountered
    ///   before reaching the end of the path.
    ///
    /// # Example
    ///
    /// ```text
    /// // Assuming the following HOCON-like structure:
    /// // {
    /// //   database: {
    /// //     connection: {
    /// //       timeout: 30
    /// //     }
    /// //   }
    /// // }
    ///
    /// let val = root.get_by_path(&["database", "connection", "timeout"]);
    /// assert_eq!(val, Some(&hocon_rs::Value::Number(30.into())));
    /// ```
    pub fn get_by_path<'a>(&self, paths: impl AsRef<[&'a str]>) -> Option<&Value> {
        let paths = paths.as_ref();

        // An empty path cannot resolve to a value
        if paths.is_empty() {
            return None;
        }

        // Start traversal from the current value
        let mut current = self;

        // Traverse the object tree step by step
        for &path in paths {
            if let Value::Object(obj) = current {
                if let Some(val) = obj.get(path) {
                    current = val;
                } else {
                    // Key not found in the current object
                    return None;
                }
            } else {
                // Current value is not an object, so the path cannot continue
                return None;
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

impl Value {
    pub fn as_bytes(&self) -> Option<BigUint> {
        fn str_to_bytes(s: &str) -> Option<BigUint> {
            let idx = s
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(s.len());
            let (num, unit) = s.split_at(idx);
            let bytes = match unit.trim() {
                "" | "B" | "b" | "byte" | "bytes" => Some(BigUint::from(1u32)),
                "kB" | "kilobyte" | "kilobytes" => Some(BigUint::from(10u32).pow(3u32)),
                "MB" | "megabyte" | "megabytes" => Some(BigUint::from(10u32).pow(6u32)),
                "GB" | "gigabyte" | "gigabytes" => Some(BigUint::from(10u32).pow(9u32)),
                "TB" | "terabyte" | "terabytes" => Some(BigUint::from(10u32).pow(12u32)),
                "PB" | "petabyte" | "petabytes" => Some(BigUint::from(10u32).pow(15u32)),
                "EB" | "exabyte" | "exabytes" => Some(BigUint::from(10u32).pow(18u32)),
                "ZB" | "zettabyte" | "zettabytes" => Some(BigUint::from(10u32).pow(21u32)),
                "YB" | "yottabyte" | "yottabytes" => Some(BigUint::from(10u32).pow(24u32)),

                "K" | "k" | "Ki" | "KiB" | "kibibyte" | "kibibytes" => {
                    Some(BigUint::from(2u32).pow(10u32))
                }
                "M" | "m" | "Mi" | "MiB" | "mebibyte" | "mebibytes" => {
                    Some(BigUint::from(2u32).pow(20u32))
                }
                "G" | "g" | "Gi" | "GiB" | "gibibyte" | "gibibytes" => {
                    Some(BigUint::from(2u32).pow(30u32))
                }
                "T" | "t" | "Ti" | "TiB" | "tebibyte" | "tebibytes" => {
                    Some(BigUint::from(2u32).pow(40u32))
                }
                "P" | "p" | "Pi" | "PiB" | "pebibyte" | "pebibytes" => {
                    Some(BigUint::from(2u32).pow(50u32))
                }
                "E" | "e" | "Ei" | "EiB" | "exbibyte" | "exbibytes" => {
                    Some(BigUint::from(2u32).pow(60u32))
                }
                "Z" | "z" | "Zi" | "ZiB" | "zebibyte" | "zebibytes" => {
                    Some(BigUint::from(2u32).pow(70u32))
                }
                "Y" | "y" | "Yi" | "YiB" | "yobibyte" | "yobibytes" => {
                    Some(BigUint::from(2u32).pow(80u32))
                }

                _ => None,
            }?;
            match BigUint::from_str(num) {
                Ok(num) => Some(&num * &bytes),
                Err(_) => match BigDecimal::from_str(num) {
                    Ok(num) => {
                        let num = &num * &bytes.to_bigint()?;
                        let (num, _) = num.with_scale(0).into_bigint_and_exponent();
                        BigUint::try_from(num).ok()
                    }
                    Err(_) => None,
                },
            }
        }
        match self {
            #[cfg(not(feature = "json_arbitrary_precision"))]
            Value::Number(num) => match num.as_u64().map(BigUint::from) {
                None => {
                    use bigdecimal::FromPrimitive;
                    let (num, _) = num
                        .as_f64()
                        .and_then(BigDecimal::from_f64)?
                        .with_scale(0)
                        .into_bigint_and_exponent();
                    BigUint::try_from(num).ok()
                }
                Some(i) => Some(i),
            },
            #[cfg(feature = "json_arbitrary_precision")]
            Value::Number(i) => str_to_bytes(i.as_str()),
            Value::String(s) => str_to_bytes(s.as_str().trim()),
            _ => None,
        }
    }

    pub fn as_duration(&self) -> Option<Duration> {
        fn duration_from_minutes(min: f64) -> Duration {
            let secs = min * 60.0;
            let whole = secs.trunc() as u64;
            let nanos = (secs.fract() * 1_000_000_000.0).round() as u32;
            Duration::new(whole, nanos)
        }

        fn duration_from_millis_f64(ms: f64) -> Duration {
            let secs = (ms / 1000.0) as u64;
            let nanos = ((ms % 1000.0) * 1_000_000.0) as u32;
            Duration::new(secs, nanos)
        }

        fn str_to_duration(s: &str) -> Option<Duration> {
            let idx = s
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(s.len());
            let (num, unit) = s.split_at(idx);
            match unit {
                "ns" | "nano" | "nanos" | "nanosecond" | "nanoseconds" => {
                    Some(Duration::from_nanos(num.parse().ok()?))
                }
                "us" | "micro" | "micros" | "microsecond" | "microseconds" => {
                    Some(Duration::from_micros(num.parse().ok()?))
                }
                "" | "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => {
                    Some(duration_from_millis_f64(num.parse().ok()?))
                }
                "s" | "second" | "seconds" => {
                    let s: f64 = num.parse().ok()?;
                    Some(duration_from_millis_f64(s * 1000.0))
                }
                "m" | "minute" | "minutes" => Some(duration_from_minutes(num.parse().ok()?)),
                "h" | "hour" | "hours" => {
                    let h: f64 = num.parse().ok()?;
                    Some(duration_from_minutes(h * 60.0))
                }
                "d" | "day" | "days" => {
                    let d: f64 = num.parse().ok()?;
                    Some(duration_from_minutes(d * 60.0 * 24.0))
                }
                _ => None,
            }
        }

        match self {
            #[cfg(not(feature = "json_arbitrary_precision"))]
            Value::Number(millis) => match millis.as_u64() {
                Some(millis) => {
                    let duration = Duration::from_millis(millis);
                    Some(duration)
                }
                None => millis.as_f64().map(duration_from_millis_f64),
            },
            #[cfg(feature = "json_arbitrary_precision")]
            Value::Number(i) => str_to_duration(i.as_str()),
            Value::String(s) => str_to_duration(s.as_str().trim()),
            _ => None,
        }
    }

    pub fn as_nanos(&self) -> Option<u128> {
        self.as_duration().map(|d| d.as_nanos())
    }

    pub fn as_millis(&self) -> Option<u128> {
        self.as_duration().map(|d| d.as_millis())
    }

    pub fn as_secs(&self) -> Option<u64> {
        self.as_duration().map(|d| d.as_secs())
    }

    pub fn as_secs_f32(&self) -> Option<f32> {
        self.as_duration().map(|d| d.as_secs_f32())
    }

    pub fn as_secs_f64(&self) -> Option<f64> {
        self.as_duration().map(|d| d.as_secs_f64())
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Object(object) => {
                write!(f, "{{")?;
                join_format(
                    object.iter(),
                    f,
                    |f| write!(f, ", "),
                    |f, (k, v)| write!(f, "{k}: {v}"),
                )?;
                write!(f, "}}")?;
                Ok(())
            }
            Value::Array(array) => {
                write!(f, "[")?;
                join(array.iter(), ", ", f)?;
                write!(f, "]")?;
                Ok(())
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
                if !matches!(v, crate::merge::value::Value::None) {
                    let v: Value = v.try_into()?;
                    object.insert(k, v);
                }
            }
            Ok(Value::Object(object))
        }

        fn from_array(array: crate::merge::array::Array) -> crate::Result<Value> {
            let mut result = Vec::with_capacity(array.len());
            for ele in array.into_inner().into_iter() {
                let v = ele.into_inner();
                let v: Value = v.try_into()?;
                result.push(v);
            }
            Ok(Value::Array(result))
        }
        let value = match value {
            crate::merge::value::Value::Object(object) => {
                if object.is_unmerged() {
                    return Err(crate::error::Error::ResolveIncomplete);
                }
                from_object(object)?
            }
            crate::merge::value::Value::Array(array) => from_array(array)?,
            crate::merge::value::Value::Boolean(boolean) => Value::Boolean(boolean),
            crate::merge::value::Value::Null | crate::merge::value::Value::None => Value::Null,
            crate::merge::value::Value::String(string) => Value::String(string),
            crate::merge::value::Value::Number(number) => Value::Number(number),
            crate::merge::value::Value::Substitution(_)
            | crate::merge::value::Value::Concat(_)
            | crate::merge::value::Value::AddAssign(_)
            | crate::merge::value::Value::DelayReplacement(_) => {
                return Err(crate::error::Error::ResolveIncomplete);
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

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
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
                Ok(Number::from_f64(v)
                    .map(Value::Number)
                    .unwrap_or(Value::Null))
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
                match map.next_key::<String>()? {
                    None => Ok(Value::Object(HashMap::new())),
                    Some(first_key) => match first_key.as_str() {
                        #[cfg(feature = "json_arbitrary_precision")]
                        "$serde_json::private::Number" => {
                            let v: String = map.next_value()?;
                            let n = serde_json::Number::from_str(&v).map_err(Error::custom)?;
                            Ok(Value::Number(n))
                        }
                        _ => {
                            let mut values = HashMap::new();
                            let value = map.next_value()?;
                            values.insert(first_key, value);
                            while let Some((k, v)) = map.next_entry()? {
                                values.insert(k, v);
                            }
                            Ok(Value::Object(values))
                        }
                    },
                }
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use rstest::rstest;

    #[rstest]
    #[case(Value::Number(0.into()), Some(BigUint::from(0u32)))]
    #[case(Value::Number(42.into()), Some(BigUint::from(42u32)))]
    #[case(Value::String("123".into()), Some(BigUint::from(123u32)))]
    #[case(Value::String("123B".into()), Some(BigUint::from(123u32)))]
    #[case(Value::String("10bytes".into()), Some(BigUint::from(10u32)))]
    #[case(Value::String("1kB".into()), Some(BigUint::from(1000u32)))]
    #[case(Value::String("2MB".into()), Some(BigUint::from(2_000_000u32)))]
    #[case(Value::String("3GB".into()), Some(BigUint::from(3_000_000_000u64)))]
    #[case(Value::String("4TB".into()), Some(BigUint::from(4_000_000_000_000u64)))]
    #[case(Value::String("5PB".into()), Some(BigUint::from(5_000_000_000_000_000u64)))]
    #[case(Value::String("6EB".into()), Some(BigUint::from(6u128 * 10u128.pow(18))))]
    #[case(Value::String("7ZB".into()), Some(BigUint::from(7u128 * 10u128.pow(21))))]
    #[case(Value::String("8YB".into()), Some(BigUint::from(8u128 * 10u128.pow(24))))]
    #[case(Value::String("1KiB".into()), Some(BigUint::from(1024u32)))]
    #[case(Value::String("2MiB".into()), Some(BigUint::from(2u64 * 1024 * 1024)))]
    #[case(Value::String("3GiB".into()), Some(BigUint::from(3u64 * 1024 * 1024 * 1024)))]
    #[case(Value::String("4TiB".into()), Some(BigUint::from(4u128 * (1u128 << 40))))]
    #[case(Value::String("5PiB".into()), Some(BigUint::from(5u128 * (1u128 << 50))))]
    #[case(Value::String("6EiB".into()), Some(BigUint::from(6u128 * (1u128 << 60))))]
    #[case(Value::String("7ZiB".into()), Some(BigUint::from(7u128 * (1u128 << 70))))]
    #[case(Value::String("8YiB".into()), Some(BigUint::from(8u128 * (1u128 << 80))))]
    #[case(Value::String("1.5kB".into()), Some(BigUint::from(1500u32)))] // 测试小数
    #[case(Value::String("0.5MiB".into()), Some(BigUint::from(512u32 * 1024)))] // 小数二进制单位
    #[case(Value::String("999999999YB".into()), Some(BigUint::from(999_999_999u128) * BigUint::from(10u128).pow(24)
    ))] // 巨大 SI 单位
    #[case(Value::String("999999999YiB".into()), Some(BigUint::from(999_999_999u128) * (BigUint::from(2u32).pow(80))
    ))] // 巨大二进制单位
    #[case(Value::String("not_a_number".into()), None)]
    #[case(Value::String("123unknown".into()), None)]
    fn test_as_bytes(#[case] input: Value, #[case] expected: Option<BigUint>) {
        assert_eq!(input.as_bytes(), expected);
    }

    fn si_factor(exp: u32) -> BigUint {
        BigUint::from(10u32).pow(exp)
    }

    fn bin_factor(exp: u32) -> BigUint {
        BigUint::from(2u32).pow(exp)
    }

    #[rstest]
    #[case("B", BigUint::from(1u32))]
    #[case("b", BigUint::from(1u32))]
    #[case("byte", BigUint::from(1u32))]
    #[case("bytes", BigUint::from(1u32))]
    #[case("kB", si_factor(3))]
    #[case("kilobyte", si_factor(3))]
    #[case("kilobytes", si_factor(3))]
    #[case("MB", si_factor(6))]
    #[case("megabyte", si_factor(6))]
    #[case("megabytes", si_factor(6))]
    #[case("GB", si_factor(9))]
    #[case("gigabyte", si_factor(9))]
    #[case("gigabytes", si_factor(9))]
    #[case("TB", si_factor(12))]
    #[case("terabyte", si_factor(12))]
    #[case("terabytes", si_factor(12))]
    #[case("PB", si_factor(15))]
    #[case("petabyte", si_factor(15))]
    #[case("petabytes", si_factor(15))]
    #[case("EB", si_factor(18))]
    #[case("exabyte", si_factor(18))]
    #[case("exabytes", si_factor(18))]
    #[case("ZB", si_factor(21))]
    #[case("zettabyte", si_factor(21))]
    #[case("zettabytes", si_factor(21))]
    #[case("YB", si_factor(24))]
    #[case("yottabyte", si_factor(24))]
    #[case("yottabytes", si_factor(24))]
    #[case("K", bin_factor(10))]
    #[case("k", bin_factor(10))]
    #[case("Ki", bin_factor(10))]
    #[case("KiB", bin_factor(10))]
    #[case("kibibyte", bin_factor(10))]
    #[case("kibibytes", bin_factor(10))]
    #[case("M", bin_factor(20))]
    #[case("m", bin_factor(20))]
    #[case("Mi", bin_factor(20))]
    #[case("MiB", bin_factor(20))]
    #[case("mebibyte", bin_factor(20))]
    #[case("mebibytes", bin_factor(20))]
    #[case("G", bin_factor(30))]
    #[case("g", bin_factor(30))]
    #[case("Gi", bin_factor(30))]
    #[case("GiB", bin_factor(30))]
    #[case("gibibyte", bin_factor(30))]
    #[case("gibibytes", bin_factor(30))]
    #[case("T", bin_factor(40))]
    #[case("t", bin_factor(40))]
    #[case("Ti", bin_factor(40))]
    #[case("TiB", bin_factor(40))]
    #[case("tebibyte", bin_factor(40))]
    #[case("tebibytes", bin_factor(40))]
    #[case("P", bin_factor(50))]
    #[case("p", bin_factor(50))]
    #[case("Pi", bin_factor(50))]
    #[case("PiB", bin_factor(50))]
    #[case("pebibyte", bin_factor(50))]
    #[case("pebibytes", bin_factor(50))]
    #[case("E", bin_factor(60))]
    #[case("e", bin_factor(60))]
    #[case("Ei", bin_factor(60))]
    #[case("EiB", bin_factor(60))]
    #[case("exbibyte", bin_factor(60))]
    #[case("exbibytes", bin_factor(60))]
    #[case("Z", bin_factor(70))]
    #[case("z", bin_factor(70))]
    #[case("Zi", bin_factor(70))]
    #[case("ZiB", bin_factor(70))]
    #[case("zebibyte", bin_factor(70))]
    #[case("zebibytes", bin_factor(70))]
    #[case("Y", bin_factor(80))]
    #[case("y", bin_factor(80))]
    #[case("Yi", bin_factor(80))]
    #[case("YiB", bin_factor(80))]
    #[case("yobibyte", bin_factor(80))]
    #[case("yobibytes", bin_factor(80))]
    fn test_as_bytes_all_units(#[case] unit: &str, #[case] factor: BigUint) {
        let input = Value::String(format!("1{}", unit));
        assert_eq!(input.as_bytes(), Some(factor));
    }

    #[rstest]
    #[case(Value::String("123 B".into()), Some(BigUint::from(123u32)))]
    #[case(Value::String("1 kB".into()), Some(BigUint::from(1000u32)))]
    #[case(Value::String("2 MB".into()), Some(BigUint::from(2_000_000u32)))]
    #[case(Value::String("1.5 kB".into()), Some(BigUint::from(1500u32)))]
    #[case(Value::String("0.5 MiB".into()), Some(BigUint::from(512u32 * 1024)))]
    fn test_as_bytes_with_space(#[case] input: Value, #[case] expected: Option<BigUint>) {
        assert_eq!(input.as_bytes(), expected);
    }

    #[rstest]
    #[case(Number::from(-1), None)]
    #[case(Number::from(0), Some(BigUint::from(0u32)))]
    #[case(Number::from(42), Some(BigUint::from(42u32)))]
    #[case(Number::from(u64::MAX), Some(BigUint::from(u64::MAX)))]
    #[case(Number::from_f64(1.1).unwrap(), Some(BigUint::from(1u32)))]
    #[case(Number::from_f64(1.9).unwrap(), Some(BigUint::from(1u32)))]
    fn test_as_bytes_number(#[case] num: Number, #[case] expected: Option<BigUint>) {
        let input = Value::Number(num);
        assert_eq!(input.as_bytes(), expected);
    }

    #[cfg(feature = "json_arbitrary_precision")]
    #[rstest]
    #[case("184467440737095516160000")]
    fn test_as_bytes_arbitrary_precision(#[case] big_num_str: &str) {
        let num: Number = serde_json::from_str(big_num_str).unwrap();

        let input = Value::Number(num);
        let expected = BigUint::parse_bytes(big_num_str.as_bytes(), 10);
        assert_eq!(input.as_bytes(), expected);
    }

    #[rstest]
    #[case(Value::String("123ms".into()), Some(123))]
    #[case(Value::String("1.5s".into()), Some(1500))]
    #[case(Value::String("1".into()), Some(1))]
    #[case(Value::String("2m".into()), Some(120_000))]
    #[case(Value::String("1.2h".into()), Some(4_320_000))]
    #[case(Value::String("1d".into()), Some(86_400_000))]
    #[case(Value::Number(Number::from_f64(2500.0).unwrap()), Some(2500))] // 2500ms
    #[case(Value::String("999us".into()), Some(0))] // <1ms 舍入为 0 毫秒
    #[case(Value::Null, None)]
    fn test_as_millis(#[case] v: Value, #[case] expected: Option<u128>) {
        assert_eq!(v.as_millis(), expected);
    }

    #[rstest]
    #[case(Value::String("2s".into()), Some(2))]
    #[case(Value::String("1.5m".into()), Some(90))]
    #[case(Value::String("0.5h".into()), Some(1800))]
    #[case(Value::String("0.1d".into()), Some(8640))]
    fn test_as_secs(#[case] v: Value, #[case] expected: Option<u64>) {
        assert_eq!(v.as_secs(), expected);
    }

    #[rstest]
    #[case(Value::String("1ns".into()), Some(1))]
    #[case(Value::String("1us".into()), Some(1000))]
    #[case(Value::String("1ms".into()), Some(1_000_000))]
    #[case(Value::String("1s".into()), Some(1_000_000_000))]
    #[case(Value::String("1m".into()), Some(60_000_000_000))]
    fn test_as_nanos(#[case] v: Value, #[case] expected: Option<u128>) {
        assert_eq!(v.as_nanos(), expected);
    }

    #[rstest]
    #[case(Value::String("2s".into()), Some(2.0))]
    #[case(Value::String("1.5s".into()), Some(1.5))]
    #[case(Value::String("1.2m".into()), Some(72.0))]
    fn test_as_secs_f32(#[case] v: Value, #[case] expected: Option<f32>) {
        assert!((v.as_secs_f32().unwrap() - expected.unwrap()).abs() < f32::EPSILON);
    }

    #[rstest]
    #[case(Value::String("2s".into()), Some(2.0))]
    #[case(Value::String("1.5s".into()), Some(1.5))]
    #[case(Value::String("1.2m".into()), Some(72.0))]
    fn test_as_secs_f64(#[case] v: Value, #[case] expected: Option<f64>) {
        assert!((v.as_secs_f64().unwrap() - expected.unwrap()).abs() < f64::EPSILON);
    }

    #[cfg(feature = "json_arbitrary_precision")]
    #[rstest]
    #[case("12300", Some(12300))]
    #[case("1.2", Some(1))]
    fn test_as_millis_arbitrary_precision(#[case] duration: &str, #[case] expected: Option<u128>) {
        let num: Number = serde_json::from_str(duration).unwrap();

        let input = Value::Number(num);
        assert_eq!(input.as_millis(), expected);
    }

    fn obj(entries: Vec<(&str, Value)>) -> Value {
        let mut map = HashMap::new();
        for (k, v) in entries {
            map.insert(k.to_string(), v);
        }
        Value::Object(map)
    }

    #[rstest]
    #[case(Value::Array(vec![Value::String("a".into()), Value::String("b".into())]),
             Some(vec![Value::String("a".into()), Value::String("b".into())]))]
    #[case(obj(vec![("0", Value::String("x".into())),
                      ("1", Value::String("y".into()))]),
             Some(vec![Value::String("x".into()), Value::String("y".into())]))]
    #[case(obj(vec![("0", Value::String("first".into())),
                      ("2", Value::String("third".into())),
                      ("1", Value::String("second".into()))]),
             Some(vec![Value::String("first".into()),
                       Value::String("second".into()),
                       Value::String("third".into())]))]
    #[case(obj(vec![("0", Value::String("ok".into())),
                      ("foo", Value::String("ignored".into()))]),
             Some(vec![Value::String("ok".into())]))]
    #[case(Value::String("not an array or object".into()), None)]
    fn test_as_array_numerically(#[case] input: Value, #[case] expected: Option<Vec<Value>>) {
        let expected = expected.as_ref().map(|v| v.iter().collect::<Vec<_>>());
        let result = input.as_array_numerically();
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(Value::Boolean(true), Some(true))]
    #[case(Value::Boolean(false), Some(false))]
    #[case(Value::String("true".into()), Some(true))]
    #[case(Value::String("on".into()), Some(true))]
    #[case(Value::String("yes".into()), Some(true))]
    #[case(Value::String("false".into()), Some(false))]
    #[case(Value::String("off".into()), Some(false))]
    #[case(Value::String("no".into()), Some(false))]
    #[case(Value::String("True".into()), None)] // case-sensitive
    #[case(Value::String("1".into()), None)] // not accepted
    #[case(Value::String("maybe".into()), None)] // invalid
    #[case(Value::Array(vec![]), None)] // wrong type
    fn test_as_boolean(#[case] input: Value, #[case] expected: Option<bool>) {
        assert_eq!(input.as_boolean(), expected);
    }

    #[rstest]
    #[case(Value::Null, true)]
    #[case(Value::String("null".into()), true)]
    #[case(Value::String("Null".into()), false)] // case-sensitive
    #[case(Value::String("NULL".into()), false)]
    #[case(Value::Boolean(false), false)]
    #[case(Value::Array(vec![]), false)]
    #[case(Value::Object(Default::default()), false)]
    fn test_is_null(#[case] input: Value, #[case] expected: bool) {
        assert_eq!(input.is_null(), expected);
    }

    #[rstest]
    // Case 1: Simple fallback
    #[case(
            obj(vec![("a", Value::String("keep".into()))]),
            obj(vec![("b", Value::String("fallback".into()))]),
            obj(vec![
                ("a", Value::String("keep".into())),
                ("b", Value::String("fallback".into()))
            ])
        )]
    // Case 2: Conflict on primitive key -> self wins
    #[case(
            obj(vec![("a", Value::String("self".into()))]),
            obj(vec![("a", Value::String("fallback".into()))]),
            obj(vec![("a", Value::String("self".into()))])
        )]
    // Case 3: Nested objects -> deep merge
    #[case(
            obj(vec![("nested", obj(vec![("x", Value::String("1".into()))]))]),
            obj(vec![("nested", obj(vec![("y", Value::String("2".into()))]))]),
            obj(vec![("nested", obj(vec![
                ("x", Value::String("1".into())),
                ("y", Value::String("2".into()))
            ]))])
        )]
    // Case 4: Self is non-object -> fallback ignored
    #[case(Value::String("self".into()), obj(vec![("a", Value::String("fb".into()))]), Value::String("self".into()))]
    // Case 5: Empty object -> fallback copied over
    #[case(obj(vec![]), obj(vec![("z", Value::String("fb".into()))]), obj(vec![("z", Value::String("fb".into()))]))]
    // Case 6: Multi-level nested merge
    #[case(
            obj(vec![("level1", obj(vec![
                ("level2", obj(vec![
                    ("key1", Value::String("self".into()))
                ]))
            ]))]),
            obj(vec![("level1", obj(vec![
                ("level2", obj(vec![
                    ("key2", Value::String("fallback".into()))
                ]))
            ]))]),
            obj(vec![("level1", obj(vec![
                ("level2", obj(vec![
                    ("key1", Value::String("self".into())),
                    ("key2", Value::String("fallback".into()))
                ]))
            ]))])
        )]
    // Case 7: Primitive in self vs. object in fallback -> self wins
    #[case(
           obj(vec![("conflict", Value::String("primitive".into()))]),
           obj(vec![("conflict", obj(vec![("nested", Value::String("fb".into()))]))]),
           obj(vec![("conflict", Value::String("primitive".into()))])
       )]
    fn test_with_fallback(#[case] base: Value, #[case] fallback: Value, #[case] expected: Value) {
        let result = base.with_fallback(fallback);
        assert_eq!(result, expected);
    }
}
