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
    use super::*;
    use crate::Config;
    use std::collections::HashMap;

    const CONFIG: &str = r#"
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
        "#;

    fn make_test_value() -> Value {
        Config::from_str(CONFIG, None).unwrap()
    }

    #[test]
    fn test_index_str_valid_and_invalid() {
        let value = make_test_value();

        // 有效访问
        assert_eq!(
            value["a"]["b"][0],
            Value::Number(serde_json::Number::from_f64(1.0).unwrap())
        );
        assert_eq!(value["a"]["b"][1]["d"], Value::String("hello".into()));
        assert_eq!(
            value["a"]["b"][1]["c"][0]["mikai"],
            Value::Number(233.into())
        );

        // 无效访问（不存在的 key）返回 Null
        assert_eq!(value["a"]["b"][1]["no_such_key"], Value::Null);

        // 对非对象使用字符串索引，返回 Null
        assert_eq!(value["a"]["b"][0]["not_object"], Value::Null);
    }

    #[test]
    fn test_index_usize_valid_and_invalid() {
        let value = make_test_value();

        // 数组越界访问返回 Null
        assert_eq!(value["a"]["b"][10], Value::Null);

        // 非数组使用数字索引，返回 Null
        assert_eq!(value["a"]["b"][1]["d"][0], Value::Null);
    }

    #[test]
    #[should_panic(expected = "cannot access index 2 of HOCON array of length 2")]
    fn test_index_or_insert_usize_out_of_bounds_panics() {
        let mut value = make_test_value();
        let _ = &mut value["a"]["b"][2]; // 超出数组长度，panic
    }

    #[test]
    #[should_panic(expected = "cannot access index 0 of HOCON object")]
    fn test_index_or_insert_usize_on_object_panics() {
        let mut value = make_test_value();
        let _ = &mut value["a"][0]; // 对 object 用数字索引
    }

    #[test]
    #[should_panic(expected = "cannot access key")]
    fn test_index_or_insert_str_on_array_panics() {
        let mut value = make_test_value();
        let _ = &mut value["a"]["b"][0]["new_key"]; // 对 number 用 str 索引
    }

    #[test]
    fn test_index_or_insert_str_on_null_creates_object() {
        let mut v = Value::Null;
        v["hello"] = Value::String("world".into());
        assert_eq!(v["hello"], Value::String("world".into()));
    }

    #[test]
    fn test_index_string_and_reference_equivalence() {
        let value = make_test_value();
        let key = "a".to_string();
        let key_ref: &str = "a";

        assert_eq!(value[&key], value["a"]);
        assert_eq!(value[&key_ref], value["a"]);
    }

    #[test]
    fn test_index_mut_inserts_new_field() {
        let mut value = Value::Object(HashMap::new());
        value["new_field"] = Value::String("hi".into());
        assert_eq!(value["new_field"], Value::String("hi".into()));
    }

    #[test]
    fn test_type_display() {
        let vals = vec![
            Value::Null,
            Value::Boolean(true),
            Value::Number(serde_json::Number::from_f64(3.14).unwrap()),
            Value::String("abc".into()),
            Value::Array(vec![]),
            Value::Object(HashMap::new()),
        ];
        let expected = ["null", "boolean", "number", "string", "array", "object"];
        for (v, exp) in vals.into_iter().zip(expected) {
            assert_eq!(format!("{}", Type(&v)), exp);
        }
    }

    #[test]
    fn test_usize_index_into_mut_valid_and_invalid() {
        let mut arr = Value::Array(vec![Value::Number(1.into()), Value::Number(2.into())]);
        // 有效访问
        let i = 1usize;
        assert!(i.index_into_mut(&mut arr).is_some());
        // 非数组类型返回 None
        let mut non_array = Value::String("not array".into());
        assert!(i.index_into_mut(&mut non_array).is_none());
    }

    #[test]
    fn test_str_index_into_mut_valid_and_invalid() {
        let mut obj = Value::Object(HashMap::from([(
            "x".to_string(),
            Value::String("ok".into()),
        )]));
        let key = "x";
        assert!(key.index_into_mut(&mut obj).is_some());
        let mut non_obj = Value::Array(vec![]);
        assert!(key.index_into_mut(&mut non_obj).is_none());
    }

    #[test]
    fn test_string_index_into_mut_and_index_or_insert() {
        let mut obj = Value::Object(HashMap::new());
        let k = "new".to_string();
        // index_into_mut
        assert!(k.index_into_mut(&mut obj).is_none());
        // index_or_insert 应插入
        k.index_or_insert(&mut obj);
        assert!(matches!(obj["new"], Value::Null));
    }

    #[test]
    fn test_ref_index_into_mut_for_string() {
        let mut obj = Value::Object(HashMap::from([("k".to_string(), Value::Number(10.into()))]));
        let k = "k".to_string();
        let ref_k = &k;
        let result = ref_k.index_into_mut(&mut obj);
        assert!(result.is_some());
    }
}
