use crate::path::Path;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use derive_more::{Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut)]
pub struct Concat(Vec<RawValue>);

impl Concat {
    pub fn new<I>(values: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item=RawValue>,
    {
        let concat = Self(values.into_iter().collect());
        for v in &concat.0 {
            if matches!(v, RawValue::Concat(_)) || matches!(v, RawValue::AddAssign(_)) {
                return Err(crate::error::Error::InvalidValue {
                    val: v.ty(),
                    ty: "concat",
                });
            }
        }
        Ok(concat)
    }

    pub fn merge(self, path: &Path) -> RawValue {
        let mut results = Vec::new();
        let mut curr: Option<RawValue> = None;
        for v2 in self.0.into_iter() {
            match curr {
                None => {
                    curr = Some(v2);
                }
                Some(v1) => {
                    match Self::merge_concat(v1, v2, path) {
                        Ok(v) => {
                            curr = Some(v);
                        }
                        Err((a, b)) => {
                            assert!(matches!(a, RawValue::AddAssign(_)));
                            assert!(matches!(a, RawValue::Concat(_)));
                            assert!(matches!(b, RawValue::AddAssign(_)));
                            assert!(matches!(b, RawValue::Concat(_)));
                            curr = Some(b);
                            results.push(a.merge(path));
                        }
                    }
                }
            }
        }
        if let Some(v) = curr {
            results.push(v.merge(path));
        }
        if results.len() == 1 {
            results.pop().unwrap()
        } else {
            RawValue::concat(results)
        }
    }

    fn merge_concat(v1: RawValue, v2: RawValue, path: &Path) -> Result<RawValue, (RawValue, RawValue)> {
        if v1.is_simple_value() && v2.is_simple_value() {
            return Ok(RawValue::quoted_string(format!("{v1}{v2}")))
        }
        match (v1, v2) {
            (RawValue::Object(o1), RawValue::Object(o2)) => {
                Ok(RawValue::Object(RawObject::merge_object(o1, o2, path)))
            }
            (RawValue::Array(mut a1), RawValue::Array(a2)) => {
                a1.extend(a2.0);
                Ok(RawValue::Array(a1))
            }
            (v1, v2) => {
                Err((v1, v2))
            }
        }
    }
}

impl Display for Concat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.iter().join(" "))
    }
}