use std::cell::RefCell;

use crate::merge::value::Value as MValue;
use crate::value::Value;
use serde::{
    Deserializer,
    de::{DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor},
    forward_to_deserialize_any,
};

impl<'de> Deserializer<'de> for Value {
    type Error = crate::error::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Boolean(b) => visitor.visit_bool(b),
            Value::String(s) => visitor.visit_string(s),
            Value::Number(n) => n
                .deserialize_any(visitor)
                .map_err(|e| crate::error::Error::Deserialize(e.to_string())),
            Value::Array(arr) => {
                struct SeqDeserializer {
                    iter: std::vec::IntoIter<Value>,
                }
                impl<'de> SeqAccess<'de> for SeqDeserializer {
                    type Error = crate::error::Error;
                    fn next_element_seed<T>(
                        &mut self,
                        seed: T,
                    ) -> Result<Option<T::Value>, Self::Error>
                    where
                        T: DeserializeSeed<'de>,
                    {
                        match self.iter.next() {
                            Some(val) => seed.deserialize(val).map(Some),
                            None => Ok(None),
                        }
                    }
                }
                visitor.visit_seq(SeqDeserializer {
                    iter: arr.into_iter(),
                })
            }
            Value::Object(map) => {
                struct MapDeserializer {
                    iter: std::collections::hash_map::IntoIter<String, Value>,
                    value: Option<Value>,
                }
                impl<'de> MapAccess<'de> for MapDeserializer {
                    type Error = crate::error::Error;
                    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
                    where
                        K: DeserializeSeed<'de>,
                    {
                        match self.iter.next() {
                            Some((k, v)) => {
                                self.value = Some(v);
                                seed.deserialize(k.into_deserializer()).map(Some)
                            }
                            None => Ok(None),
                        }
                    }
                    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
                    where
                        V: DeserializeSeed<'de>,
                    {
                        seed.deserialize(self.value.take().unwrap())
                    }
                }
                visitor.visit_map(MapDeserializer {
                    iter: map.into_iter(),
                    value: None,
                })
            }
        }
    }

    // 我们只需要实现 `deserialize_any`，其他都用默认的转发实现即可
    forward_to_deserialize_any! {
        <W: Visitor<'de>>
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de> Deserializer<'de> for MValue {
    type Error = crate::error::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            MValue::Null | MValue::None => visitor.visit_unit(),
            MValue::Boolean(b) => visitor.visit_bool(b),
            MValue::String(s) => visitor.visit_string(s),
            MValue::Number(n) => {
                let n = n.deserialize_any(visitor)?;
                Ok(n)
            }
            MValue::Array(arr) => {
                struct SeqDeserializer {
                    iter: std::vec::IntoIter<RefCell<MValue>>,
                }
                impl<'de> SeqAccess<'de> for SeqDeserializer {
                    type Error = crate::error::Error;
                    fn next_element_seed<T>(
                        &mut self,
                        seed: T,
                    ) -> Result<Option<T::Value>, Self::Error>
                    where
                        T: DeserializeSeed<'de>,
                    {
                        match self.iter.next() {
                            Some(val) => seed.deserialize(val.into_inner()).map(Some),
                            None => Ok(None),
                        }
                    }
                }
                visitor.visit_seq(SeqDeserializer {
                    iter: arr.into_inner().into_iter(),
                })
            }
            MValue::Object(map) => {
                struct MapDeserializer {
                    iter: std::collections::btree_map::IntoIter<String, RefCell<MValue>>,
                    value: Option<RefCell<MValue>>,
                }
                impl<'de> MapAccess<'de> for MapDeserializer {
                    type Error = crate::error::Error;
                    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
                    where
                        K: DeserializeSeed<'de>,
                    {
                        match self.iter.next() {
                            Some((k, mut v)) => {
                                if matches!(v.get_mut(), MValue::None) {
                                    self.next_key_seed(seed)
                                } else {
                                    self.value = Some(v);
                                    seed.deserialize(k.into_deserializer()).map(Some)
                                }
                            }
                            None => Ok(None),
                        }
                    }
                    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
                    where
                        V: DeserializeSeed<'de>,
                    {
                        seed.deserialize(self.value.take().unwrap().into_inner())
                    }
                }
                visitor.visit_map(MapDeserializer {
                    iter: map.into_inner().into_iter(),
                    value: None,
                })
            }
            MValue::Substitution(_)
            | MValue::Concat(_)
            | MValue::AddAssign(_)
            | MValue::DelayReplacement(_) => Err(crate::error::Error::ResolveIncomplete),
        }
    }

    forward_to_deserialize_any! {
        <W: Visitor<'de>>
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Config {
        app: App,
        deployment: Deployment,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct App {
        name: String,
        version: String,
        database: Database,
        servers: Vec<Server>,
        log_dir: String,
        features: Features,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Database {
        host: String,
        port: u16,
        user: String,
        password: String,
        options: DatabaseOptions,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct DatabaseOptions {
        ssl: bool,
        timeout: u32,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Server {
        host: String,
        roles: Vec<String>,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Features {
        experimental: bool,
        max_connections: u32,
        tags: Vec<String>,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Deployment {
        replicas: u32,
        image: String,
    }
    #[test]
    fn test_de() -> crate::Result<()> {
        let config_hocon: Config =
            crate::config::Config::load("test_conf/comprehensive/deserialize.conf", None)?;
        let file = std::fs::File::open("test_conf/comprehensive/deserialize.json")?;
        let config_json: Config = serde_json::from_reader(file)?;
        assert_eq!(config_hocon, config_json);
        Ok(())
    }
}
