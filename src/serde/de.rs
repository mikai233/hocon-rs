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
                .map_err(|e| crate::error::Error::DeserializeError(e.to_string())),
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
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
