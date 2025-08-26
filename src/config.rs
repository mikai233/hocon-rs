use std::str::FromStr;

use crate::config_options::ConfigOptions;
use crate::merge::object::Object as MObject;
use crate::merge::value::Value as MValue;
use crate::parser::loader::{self, load_from_classpath, load_from_url};
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
        let raw = loader::load(path, opts.unwrap_or_default().into())?;
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
        load_from_classpath(path, opts.unwrap_or_default().into())
    }

    pub fn parse_url(
        url: impl AsRef<str>,
        opts: Option<ConfigOptions>,
    ) -> crate::Result<RawObject> {
        let url = url::Url::from_str(url.as_ref())?;
        load_from_url(url, opts.unwrap_or_default().into())
    }

    pub fn parse_map(values: std::collections::HashMap<String, Value>) -> crate::Result<Value> {
        unimplemented!()
    }

    pub fn parse_str(s: impl AsRef<str>, opts: Option<ConfigOptions>) -> crate::Result<Value> {
        unimplemented!()
    }

    pub fn to_writer<W>(&self, writer: W) -> crate::Result<()>
    where
        W: std::io::Write,
    {
        unimplemented!()
    }

    pub fn to_writer_pretty<W>(&self, writer: W) -> crate::Result<()>
    where
        W: std::io::Write,
    {
        unimplemented!()
    }

    pub fn to_string<W>(&self, pretty: bool) -> crate::Result<String> {
        unimplemented!()
    }

    pub fn to_string_pretty<W>(&self, pretty: bool) -> crate::Result<String> {
        unimplemented!()
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

    use crate::{config::Config, config_options::ConfigOptions, value::Value};

    #[test]
    fn test_path_expression_get() -> crate::Result<()> {
        let value1 = Value::with_object([
            ("a", Value::new_string("hello")),
            ("b", Value::new_string("world")),
        ]);
        let value2 = Value::with_array([Value::Number(1.into()), Value::Number(2.into())]);
        let value2 = Value::with_object([("a", value1), ("b", value2)]);
        let value3 = Value::with_object([("a", value2)]);
        let object = value3.into_object().unwrap();
        Ok(())
    }

    #[test]
    fn aa() {
        let mut options = ConfigOptions::default();
        options.classpath = vec!["resources".to_string()];
        options.use_system_environment = false;
        let r = Config::load("main", Some(options)).unwrap();
        println!("{}", r)
    }
}
