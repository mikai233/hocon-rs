use crate::config_options::ConfigOptions;
use crate::merge::object::Object as MObject;
use crate::merge::value::Value as MValue;
use crate::parser::loader::{self, load_from_path, parse_hocon};
use crate::parser::read::{StrRead, StreamRead};
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::raw::{field::ObjectField, include::Inclusion};
use crate::value::Value;
use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

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

    pub fn load<T>(
        path: impl AsRef<std::path::Path>,
        options: Option<ConfigOptions>,
    ) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        let raw = loader::load(&path, options.unwrap_or_default(), None)?;
        tracing::debug!("path: {} raw obj: {}", path.as_ref().display(), raw);
        Self::resolve_object::<T>(raw)
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

    pub fn resolve<T>(self) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        Self::resolve_object(self.object)
    }

    pub fn parse_file<T>(
        path: impl AsRef<std::path::Path>,
        opts: Option<ConfigOptions>,
    ) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        let raw = load_from_path(path, opts.unwrap_or_default(), None)?;
        Self::resolve_object::<T>(raw)
    }

    #[cfg(feature = "urls_includes")]
    pub fn parse_url<T>(url: impl AsRef<str>, opts: Option<ConfigOptions>) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        use std::str::FromStr;
        let url = url::Url::from_str(url.as_ref())?;
        let raw = loader::load_from_url(url, opts.unwrap_or_default().into())?;
        Self::resolve_object::<T>(raw)
    }

    pub fn parse_map<T>(values: std::collections::HashMap<String, Value>) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
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
                Value::Array(array) => RawValue::array(array.into_iter().map(into_raw).collect()),
                Value::Boolean(boolean) => RawValue::Boolean(boolean),
                Value::Null => RawValue::Null,
                Value::String(string) => {
                    let s = RawString::path_expression(
                        string.split('.').map(RawString::quoted).collect(),
                    );
                    RawValue::String(s)
                }
                Value::Number(number) => RawValue::Number(number),
            }
        }
        let raw = into_raw(Value::Object(ahash::HashMap::from_iter(values)));
        if let RawValue::Object(raw_obj) = raw {
            Self::resolve_object::<T>(raw_obj)
        } else {
            unreachable!("raw should always be an object");
        }
    }

    pub fn parse_str<T>(s: &str, options: Option<ConfigOptions>) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        let read = StrRead::new(s);
        let raw = parse_hocon(read, options.unwrap_or_default(), None)?;
        Self::resolve_object::<T>(raw)
    }

    pub fn parse_reader<R, T>(rdr: R, options: Option<ConfigOptions>) -> crate::Result<T>
    where
        R: std::io::Read,
        T: DeserializeOwned,
    {
        use crate::parser::read::DEFAULT_BUFFER;
        let read: StreamRead<_, DEFAULT_BUFFER> = StreamRead::new(std::io::BufReader::new(rdr));
        let raw = parse_hocon(read, options.unwrap_or_default(), None)?;
        Self::resolve_object::<T>(raw)
    }

    fn resolve_object<T>(object: RawObject) -> crate::Result<T>
    where
        T: DeserializeOwned,
    {
        let object = MObject::from_raw(None, object)?;
        let mut value = MValue::Object(object);
        value.resolve()?;
        if value.is_unmerged() {
            return Err(crate::error::Error::ResolveIncomplete);
        }
        let value: Value = value.try_into()?;
        T::deserialize(value)
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
    use crate::Result;
    use crate::error::Error;
    use crate::{config::Config, config_options::ConfigOptions, value::Value};
    use rstest::rstest;

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
    #[case("resources/concat2.conf", "resources/concat2.json")]
    #[case("resources/concat3.conf", "resources/concat3.json")]
    #[case("resources/concat4.conf", "resources/concat4.json")]
    #[case("resources/concat5.conf", "resources/concat5.json")]
    #[case("resources/include.conf", "resources/include.json")]
    #[case("resources/comment.conf", "resources/comment.json")]
    #[case("resources/substitution.conf", "resources/substitution.json")]
    #[case("resources/self_referential.conf", "resources/self_referential.json")]
    fn test_hocon(
        #[case] hocon: impl AsRef<std::path::Path>,
        #[case] json: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()].into();
        let value = Config::load::<Value>(hocon, Some(options))?;
        let f = std::fs::File::open(json)?;
        let expected_value: serde_json::Value = serde_json::from_reader(f)?;
        let expected_value: Value = expected_value.into();
        value.assert_deep_eq(&expected_value, "$");
        Ok(())
    }

    #[test]
    fn test_max_depth() -> Result<()> {
        let error = Config::load::<Value>("resources/max_depth.conf", None)
            .err()
            .unwrap();
        assert!(matches!(error, Error::RecursionDepthExceeded { .. }));
        Ok(())
    }

    #[test]
    fn test_include_cycle() -> Result<()> {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()].into();
        let error = Config::load::<Value>("resources/include_cycle.conf", Some(options))
            .err()
            .unwrap();
        assert!(matches!(error, Error::Include { .. }));
        Ok(())
    }

    #[test]
    fn test_substitution_cycle() -> Result<()> {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()].into();
        let error = Config::load::<Value>("resources/substitution_cycle.conf", Some(options))
            .err()
            .unwrap();
        assert!(matches!(error, Error::SubstitutionCycle { .. }));
        Ok(())
    }

    #[test]
    fn test_substitution_not_found() -> Result<()> {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()].into();
        let error = Config::load::<Value>("resources/substitution2.conf", Some(options))
            .err()
            .unwrap();
        assert!(matches!(error, Error::SubstitutionNotFound { .. }));
        Ok(())
    }
}
