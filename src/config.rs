use crate::key::Key;
use crate::raw::raw_object::RawObject;
use crate::value::Value;
use ahash::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    object: RawObject,
}

impl Config {
    pub fn get(&self, key: impl Into<Key>) -> crate::Result<Option<Value>> {
        todo!()
        // let trimmed = path.as_ref().trim();
        // if trimmed.is_empty() {
        //     panic!("path is empty");
        // }
        // if trimmed.starts_with('.') {
        //     panic!("leading period '.' not allowed");
        // }
        // if trimmed.ends_with('.') {
        //     panic!("trailing period '.' not allowed");
        // }
        // if trimmed.contains("..") {
        //     panic!("adjacent periods '..' not allowed");
        // }
        //
        // let mut current = &self.value;
        // for path in trimmed.split('.') {
        //     match current.as_object() {
        //         None => {
        //             return None
        //         }
        //         Some(object) => {
        //             match object.get(path) {
        //                 None => {
        //                     return None
        //                 }
        //                 Some(value) => {
        //                     current = value;
        //                 }
        //             }
        //         }
        //     }
        // }
        // Some(current)
    }
}

impl Into<Value> for Config {
    fn into(self) -> Value {
        todo!()
    }
}

impl From<HashMap<String, Value>> for Config {
    fn from(value: HashMap<String, Value>) -> Self {
        todo!()
    }
}

impl From<RawObject> for Config {
    fn from(value: RawObject) -> Self {
        todo!()
    }
}


// impl Index<&str> for Config {
//     type Output = Value;
//
//     fn index(&self, key: &str) -> &Self::Output {
//         self.get(key).expect("no entry found for key")
//     }
// }
//
// impl From<Object> for Config {
//     fn from(value: Object) -> Self {
//         Self {
//             value: Value::Object(value),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::value::Value;

    #[test]
    fn test_path_expression_get() -> crate::Result<()> {
        let value1 = Value::with_object([("a", Value::new_string("hello")), ("b", Value::new_string("world"))]);
        let value2 = Value::with_array([Value::Number(1.into()), Value::Number(2.into())]);
        let value2 = Value::with_object([("a", value1), ("b", value2)]);
        let value3 = Value::with_object([("a", value2)]);
        let object = value3.into_object().unwrap();
        let config = Config::from(object);
        let v = config.get("a.a.b")?.unwrap();
        assert_eq!(v, "world".into());
        Ok(())
    }
}