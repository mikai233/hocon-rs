use crate::path::Path;
use crate::raw::field::ObjectField;
use crate::raw::include::Inclusion;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::value::Value;
use serde::Deserialize;

#[derive(Debug)]
pub struct ConfigFactory {
    pub object: RawObject,
}

impl ConfigFactory {
    pub fn load() {
        todo!()
    }

    pub fn add<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<RawString>,
        V: Into<RawValue>,
    {
        let field = ObjectField::key_value(key, value);
        self.object.push(field);
        self
    }

    pub fn include(&mut self, inclusion: Inclusion) -> &mut Self {
        let field = ObjectField::inclusion(inclusion);
        self.object.push(field);
        self
    }

    pub fn resolve(&self) -> crate::Result<Value> {
        unimplemented!()
    }

    pub fn deserialize<'de, T>(&'de self, path: Option<Path>) -> crate::Result<T>
    where
        T: Deserialize<'de>,
    {
        unimplemented!()
    }
}