use super::Value;
use core::fmt::{self, Display};
use core::ops;
use std::collections::HashMap;

/// A trait used to index into a HOCON [`Value`].
///
/// This trait is sealed and cannot be implemented outside this crate.
/// Implementations are provided for:
/// - `usize` for indexing into arrays
/// - `str` and `String` for indexing into objects by key
/// - `&T` where `T: Index` for convenience
///
/// The behavior is similar to `serde_json::value::Index`.
pub trait Index: private::Sealed {
    /// Attempt to index into an immutable [`Value`], returning `Some(&Value)` if
    /// the index is valid, otherwise `None`.
    ///
    /// - For `usize`, this looks up an array element.
    /// - For `str` / `String`, this looks up an object field.
    /// - For invalid cases, returns `None`.
    #[doc(hidden)]
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value>;

    /// Attempt to index into a mutable [`Value`], returning `Some(&mut Value)`
    /// if the index is valid, otherwise `None`.
    ///
    /// - For `usize`, this gives mutable access to an array element.
    /// - For `str` / `String`, this gives mutable access to an object field.
    /// - For invalid cases, returns `None`.
    #[doc(hidden)]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value>;

    /// Index into a mutable [`Value`], inserting if necessary, and return
    /// a mutable reference.
    ///
    /// - For `str` / `String`, if the `Value` is `Null`, it will be replaced
    ///   with an empty object before inserting the new key.
    /// - For `usize`, this will panic if the index is out of bounds or if the
    ///   `Value` is not an array.
    /// - For `str` / `String`, this will panic if the `Value` is not an object.
    #[doc(hidden)]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value;
}

impl Index for usize {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        match v {
            Value::Array(vec) => vec.get(*self),
            _ => None,
        }
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        match v {
            Value::Array(vec) => vec.get_mut(*self),
            _ => None,
        }
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        match v {
            Value::Array(vec) => {
                let len = vec.len();
                vec.get_mut(*self).unwrap_or_else(|| {
                    panic!(
                        "cannot access index {} of HOCON array of length {}",
                        self, len
                    )
                })
            }
            _ => panic!("cannot access index {} of HOCON {}", self, Type(v)),
        }
    }
}

impl Index for str {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        match v {
            Value::Object(map) => map.get(self),
            _ => None,
        }
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        match v {
            Value::Object(map) => map.get_mut(self),
            _ => None,
        }
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        if let Value::Null = v {
            *v = Value::Object(HashMap::new());
        }
        match v {
            Value::Object(map) => map.entry(self.to_owned()).or_insert(Value::Null),
            _ => panic!("cannot access key {:?} in HOCON {}", self, Type(v)),
        }
    }
}

impl Index for String {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        self[..].index_into(v)
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        self[..].index_into_mut(v)
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        self[..].index_or_insert(v)
    }
}

impl<T> Index for &T
where
    T: ?Sized + Index,
{
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).index_into(v)
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        (**self).index_into_mut(v)
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        (**self).index_or_insert(v)
    }
}

// Private sealing to prevent external implementations
mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl<T> Sealed for &T where T: ?Sized + Sealed {}
}

/// Pretty-print type information for error messages.
struct Type<'a>(&'a Value);

impl<'a> Display for Type<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Value::Null => f.write_str("null"),
            Value::Boolean(_) => f.write_str("boolean"),
            Value::Number(_) => f.write_str("number"),
            Value::String(_) => f.write_str("string"),
            Value::Array(_) => f.write_str("array"),
            Value::Object(_) => f.write_str("object"),
        }
    }
}

impl<I> ops::Index<I> for Value
where
    I: Index,
{
    type Output = Value;

    /// Immutable indexing operator (`value[index]`).
    ///
    /// Returns a reference to the indexed value, or `Value::Null` if the index
    /// is not present or invalid.
    fn index(&self, index: I) -> &Value {
        static NULL: Value = Value::Null;
        index.index_into(self).unwrap_or(&NULL)
    }
}

impl<I> ops::IndexMut<I> for Value
where
    I: Index,
{
    /// Mutable indexing operator (`value[index] = ...`).
    ///
    /// Will insert new entries for string keys into objects.
    /// For `usize` indices, will panic if the index is out of bounds.
    /// Panics if the type of the value does not match the index type.
    fn index_mut(&mut self, index: I) -> &mut Value {
        index.index_or_insert(self)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::Result;
    use crate::index::Type;
    use crate::{Config, Value};
    const CONFIG: &str = r#"
root = {
  a = {
    b = [
      1.0,
      {
        c = [
          {mikai = 233}
        ],
        d = "hello",
      },
    ]
  }
}
        "#;

    #[test]
    fn test_index() -> Result<()> {
        let mut value: Value = Config::parse_str(CONFIG, None)?;
        let v = &value["root"]["a"]["b"][0];
        assert_eq!(v.as_f64(), Some(1.0));
        let v = &value["root"]["a"]["b"][1]["c"][0]["mikai"];
        assert_eq!(v.as_i64(), Some(233));
        value["root"]["a"]["b"][1]["c"][0]["mikai"] = Value::Number(39.into());
        let v = &value["root"]["a"]["b"][1]["c"][0]["mikai"];
        assert_eq!(v.as_i64(), Some(39));
        let v = &value["root"]["a"]["b"][1]["d"];
        assert_eq!(v.as_str(), Some("hello"));
        value["root"]["a"]["b"][1]["d"] = Value::String("world".to_string());
        let v = &value["root"]["a"]["b"][1]["d"];
        assert_eq!(v.as_str(), Some("world"));
        Ok(())
    }

    #[test]
    fn test_type() {
        assert_eq!(Type(&Value::Null).to_string(), "null");
        assert_eq!(Type(&Value::Array(vec![])).to_string(), "array");
        assert_eq!(Type(&Value::Object(HashMap::new())).to_string(), "object");
        assert_eq!(Type(&Value::Boolean(false)).to_string(), "boolean");
        assert_eq!(Type(&Value::String("".to_string())).to_string(), "string");
        assert_eq!(Type(&Value::Number(0.into())).to_string(), "number");
    }
}
