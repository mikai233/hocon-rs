use std::str::FromStr;

use crate::config_options::ConfigOptions;
use crate::merge::object::Object as MObject;
use crate::merge::value::Value as MValue;
use crate::parser::loader::{self, load_from_path, load_from_url, load_hocon};
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::raw::{field::ObjectField, include::Inclusion};
use crate::value::Value;
use derive_more::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut)]
pub struct Config {
    #[deref]
    #[deref_mut]
    object: RawObject,
    options: ConfigOptions,
}

impl Config {
    pub fn new(options: Option<ConfigOptions>) -> Self {
        Self {
            object: Default::default(),
            options: options.unwrap_or_default(),
        }
    }

    pub fn load(
        path: impl AsRef<std::path::Path>,
        opts: Option<ConfigOptions>,
    ) -> crate::Result<Value> {
        let raw = loader::load(&path, opts.unwrap_or_default().into())?;
        tracing::debug!("path: {} raw obj: {}", path.as_ref().display(), raw);
        let value = Self::resolve_object(raw)?;
        Ok(value)
    }

    pub fn add_kv<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<RawString>,
        V: Into<RawValue>,
    {
        let field = ObjectField::key_value(key, value);
        self.object.push(field);
        self
    }

    pub fn add_include(&mut self, inclusion: Inclusion) -> &mut Self {
        let field = ObjectField::inclusion(inclusion);
        self.object.push(field);
        self
    }

    pub fn add_kvs<I, V>(&mut self, kvs: I) -> &mut Self
    where
        I: IntoIterator<Item = (String, V)>,
        V: Into<RawValue>,
    {
        let fields = kvs
            .into_iter()
            .map(|(key, value)| ObjectField::key_value(key, value));
        self.object.extend(fields);
        self
    }

    pub fn add_object(&mut self, object: RawObject) -> &mut Self {
        self.object.extend(object.0);
        self
    }

    pub fn resolve(self) -> crate::Result<Value> {
        let object = crate::merge::object::Object::from_raw(None, self.object)?;
        let mut value = crate::merge::value::Value::Object(object);
        value.resolve()?;
        let value: Value = value.try_into()?;
        Ok(value)
    }

    pub fn parse_file(
        path: impl AsRef<std::path::Path>,
        opts: Option<ConfigOptions>,
    ) -> crate::Result<RawObject> {
        load_from_path(path, opts.unwrap_or_default().into())
    }

    pub fn parse_url(
        url: impl AsRef<str>,
        opts: Option<ConfigOptions>,
    ) -> crate::Result<RawObject> {
        let url = url::Url::from_str(url.as_ref())?;
        load_from_url(url, opts.unwrap_or_default().into())
    }

    pub fn parse_map(values: std::collections::HashMap<String, Value>) -> crate::Result<Value> {
        fn into_raw(value: Value) -> RawValue {
            match value {
                Value::Object(object) => {
                    let len = object.len();
                    let fields = object.into_iter().fold(
                        Vec::with_capacity(len),
                        |mut acc, (key, value)| {
                            let field = ObjectField::key_value(key, into_raw(value));
                            acc.push(field);
                            acc
                        },
                    );
                    RawValue::Object(RawObject::new(fields))
                }
                Value::Array(array) => RawValue::array(array.into_iter().map(into_raw)),
                Value::Boolean(boolean) => RawValue::Boolean(boolean),
                Value::Null => RawValue::Null,
                Value::String(string) => {
                    let s = RawString::concat(string.split('.').map(|p| (p.into(), Some("."))));
                    RawValue::String(s)
                }
                Value::Number(number) => RawValue::Number(number),
            }
        }
        let raw = into_raw(Value::Object(ahash::HashMap::from_iter(values.into_iter())));
        let value = Self::resolve_object(raw.try_into()?)?;
        Ok(value)
    }

    pub fn parse_str(s: impl AsRef<str>, opts: Option<ConfigOptions>) -> crate::Result<Value> {
        let parse_opts = opts.map(Into::into).unwrap_or_default();
        let raw = load_hocon(s, parse_opts)?;
        let value = Self::resolve_object(raw)?;
        Ok(value)
    }

    pub fn resolve_object(object: RawObject) -> crate::Result<Value> {
        let object = MObject::from_raw(None, object)?;
        let mut value = MValue::Object(object);
        value.resolve()?;
        if value.is_unmerged() {
            return Err(crate::error::Error::ResolveNotComplete);
        }
        let value = value.try_into()?;
        Ok(value)
    }
}

impl From<RawObject> for Config {
    fn from(value: RawObject) -> Self {
        Config {
            object: value,
            options: Default::default(),
        }
    }
}

/// Constructs a [Config] from a [std::collections::HashMap].
///
/// Keys are treated as literal values, not path expressions.
/// For example, a key `"foo.bar"` in the map will result in a single entry
/// with the key `"foo.bar"`, rather than creating a nested object
/// with `"foo"` containing another object `"bar"`.
impl From<std::collections::HashMap<String, Value>> for Config {
    fn from(value: std::collections::HashMap<String, Value>) -> Self {
        let fields = value
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        Config {
            object: RawObject::new(fields),
            options: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::{config::Config, config_options::ConfigOptions, value::Value};

    impl Value {
        pub fn assert_deep_eq(&self, other: &Value, path: &str) {
            match (self, other) {
                (Value::Object(map1), Value::Object(map2)) => {
                    for (k, v1) in map1 {
                        let new_path = format!("{}/{}", path, k);
                        if let Some(v2) = map2.get(k) {
                            v1.assert_deep_eq(v2, &new_path);
                        } else {
                            panic!("Key missing in right: {}", new_path);
                        }
                    }
                    for k in map2.keys() {
                        if !map1.contains_key(k) {
                            panic!("Key missing in left: {}/{}", path, k);
                        }
                    }
                }
                (Value::Array(arr1), Value::Array(arr2)) => {
                    let len = arr1.len().max(arr2.len());
                    for i in 0..len {
                        let new_path = format!("{}/[{}]", path, i);
                        match (arr1.get(i), arr2.get(i)) {
                            (Some(v1), Some(v2)) => v1.assert_deep_eq(v2, &new_path),
                            (Some(_), None) => panic!("Index missing in right: {}", new_path),
                            (None, Some(_)) => panic!("Index missing in left: {}", new_path),
                            _ => {}
                        }
                    }
                }
                _ => {
                    assert_eq!(
                        self, other,
                        "Difference at {}: left={:?}, right={:?}",
                        path, self, other
                    );
                }
            }
        }
    }

    #[rstest]
    #[case("resources/empty.conf", "resources/empty.json")]
    #[case("resources/base.conf", "resources/base.json")]
    #[case("resources/add_assign.conf", "resources/add_assign_expected.json")]
    #[case("resources/concat.conf", "resources/concat.json")]
    fn test_hocon(
        #[case] hocon: impl AsRef<std::path::Path>,
        #[case] json: impl AsRef<std::path::Path>,
    ) -> crate::Result<()> {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()];
        let value = Config::load(hocon, Some(options))?;
        let f = std::fs::File::open(json).unwrap();
        let expected_value: serde_json::Value = serde_json::from_reader(f)?;
        let expected_value: Value = expected_value.into();
        value.assert_deep_eq(&expected_value, "$");
        Ok(())
    }

    #[test]
    fn test_max_depth() -> crate::Result<()> {
        let error = Config::load("resources/max_depth.conf", None)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            crate::error::Error::RecursionDepthExceeded { .. }
        ));
        Ok(())
    }
}
