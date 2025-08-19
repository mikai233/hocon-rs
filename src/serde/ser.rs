// use serde::{
//     Serialize, Serializer,
//     ser::{SerializeMap, SerializeSeq},
// };

// use crate::value::Value;

// impl Serialize for Value {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         match self {
//             Value::Null => serializer.serialize_unit(),
//             Value::Boolean(b) => serializer.serialize_bool(*b),
//             Value::String(s) => serializer.serialize_str(s),
//             Value::Number(n) => n.serialize(serializer),
//             Value::Array(vec) => {
//                 let mut seq = serializer.serialize_seq(Some(vec.len()))?;
//                 for v in vec {
//                     seq.serialize_element(v)?;
//                 }
//                 seq.end()
//             }
//             Value::Object(map) => {
//                 let mut m = serializer.serialize_map(Some(map.len()))?;
//                 for (k, v) in map {
//                     m.serialize_entry(k, v)?;
//                 }
//                 m.end()
//             }
//         }
//     }
// }
