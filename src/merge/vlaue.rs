use std::fmt::Display;

use crate::{
    merge::{
        add_assign::AddAssign, array::Array, concat::Concat, delay_merge::DelayMerge,
        object::Object, substitution::Substitution,
    },
    path::Path,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Value {
    Object(Object),
    Array(Array),
    Boolean(bool),
    #[default]
    Null,
    String(String),
    Number(serde_json::Number),
    Substitution(Substitution),
    Concat(Concat),
    AddAssign(AddAssign),
    DelayMerge(DelayMerge),
}

impl Value {
    pub(crate) fn object(o: impl Into<Object>) -> Value {
        Value::Object(o.into())
    }

    pub(crate) fn array(a: impl Into<Array>) -> Value {
        Value::Array(a.into())
    }

    pub(crate) fn string(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }

    pub(crate) fn number(n: serde_json::Number) -> Value {
        Value::Number(n)
    }

    pub(crate) fn substitution(s: impl Into<Substitution>) -> Value {
        Value::Substitution(s.into())
    }

    pub(crate) fn concat(c: impl Into<Concat>) -> Value {
        Value::Concat(c.into())
    }

    pub(crate) fn add_assign(a: impl Into<AddAssign>) -> Value {
        Value::AddAssign(a.into())
    }

    pub(crate) fn delay_merge<I>(value: I) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        let m = DelayMerge::from_iter(value);
        Value::DelayMerge(m)
    }

    pub(crate) fn ty(&self) -> &'static str {
        match self {
            Value::Object(_) => "object",
            Value::Array(_) => "array",
            Value::Boolean(_) => "boolean",
            Value::Null => "null",
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Substitution(_) => "substitution",
            Value::Concat(_) => "concat",
            Value::AddAssign(_) => "add_assign",
            Value::DelayMerge(_) => "delay_merge",
        }
    }

    /// Replaces left value to right value if they are simple values.
    /// TODO if right is add_asign and left is not array, should return error.
    pub(crate) fn replacement(left: Value, right: Value) -> crate::Result<Value> {
        let new_val = match left {
            Value::Object(mut obj_left) => match right {
                Value::Object(right) => {
                    // merge two objects
                    obj_left.merge(right)?;
                    Value::object(obj_left)
                }
                Value::Array(_)
                | Value::Boolean(_)
                | Value::Null
                | Value::String(_)
                | Value::Number(_) => right,
                Value::Substitution(_) => {
                    let left = Value::object(obj_left);
                    Value::delay_merge(vec![left, right])
                }
                Value::Concat(mut concat) => {
                    if concat
                        .iter()
                        .all(|v| matches!(v, Value::Object(_) | Value::Substitution(_)))
                    {
                        // the concat result maybe an object, so we need to push the left object for potential
                        // object concat
                        let left = Value::object(obj_left);
                        concat.push_front(left);
                        Value::concat(concat)
                    } else {
                        // the concat result must be a quoted string or a array, it will override the left value
                        Value::concat(concat)
                    }
                    // if there is any bug here, for safety's side, jsut push the left value into the front
                }
                Value::AddAssign(_) => {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: "object",
                        ty2: "array",
                    });
                }
                Value::DelayMerge(mut delay_merge) => {
                    let left = Value::object(obj_left);
                    delay_merge.push_front(left);
                    Value::DelayMerge(delay_merge)
                }
            },
            Value::Array(mut array) => {
                if let Value::AddAssign(add_assign) = right {
                    array.push(add_assign.into());
                    Value::array(array)
                } else {
                    right
                }
            }
            Value::Boolean(_)
            | Value::Null
            | Value::String(_)
            | Value::Number(_)
            | Value::AddAssign(_) => right,
            Value::Substitution(_) |
            // FIXME Is there could be another DelayMerge here?
            Value::Concat(_) |
            Value::DelayMerge(_) => {
                Value::delay_merge(vec![left,right])
            }
        };
        Ok(new_val)
    }

    pub(crate) fn concatenate(left: Value, right: Value) -> crate::Result<Value> {
        let val = match left {
            Value::Object(mut left_obj) => match right {
                Value::Null => Value::object(left_obj),
                Value::Object(right_obj) => {
                    left_obj.merge(right_obj)?;
                    Value::object(left_obj)
                }
                Value::Array(_) | Value::Boolean(_) | Value::String(_) | Value::Number(_) => {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: "object",
                        ty2: right.ty(),
                    });
                }
                Value::Substitution(_) => {
                    let left = Value::object(left_obj);
                    Value::delay_merge(vec![left, right])
                }
                Value::Concat(concat) => {
                    let left = Value::object(left_obj);
                    let right = concat.reslove()?;
                    Self::concatenate(left, right)?
                }
                Value::AddAssign(_) => {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: "object",
                        ty2: right.ty(),
                    });
                }
                Value::DelayMerge(_) => {
                    let left = Value::object(left_obj);
                    Value::delay_merge(vec![left, right])
                }
            },
            Value::Array(mut left_array) => {
                if let Value::Array(right_array) = right {
                    left_array.extend(right_array.0);
                    Value::array(left_array)
                } else {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: "array",
                        ty2: right.ty(),
                    });
                }
            }
            Value::Null => right,
            Value::Boolean(_) | Value::String(_) | Value::Number(_) => {
                if matches!(
                    right,
                    Value::Boolean(_) | Value::String(_) | Value::Number(_)
                ) {
                    Value::string(format!("{left}{right}"))
                } else {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: left.ty(),
                        ty2: right.ty(),
                    });
                }
            }
            Value::Substitution(_) => Value::delay_merge(vec![left, right]),
            Value::Concat(concat) => {
                let left = concat.reslove()?;
                Self::concatenate(left, right)?
            }
            Value::AddAssign(_) => {
                return Err(crate::error::Error::ConcatenationDifferentType {
                    ty1: left.ty(),
                    ty2: right.ty(),
                });
            }
            Value::DelayMerge(_) => Value::delay_merge(vec![left, right]),
        };
        Ok(val)
    }

    pub(crate) fn is_merged(&self) -> bool {
        match self {
            Value::Object(object) => matches!(object, Object::Merged(_)),
            Value::Array(array) => array.is_merged(),
            Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => true,
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayMerge(_) => false,
        }
    }

    pub(crate) fn get_by_path(&self, path: Option<&Path>) -> Option<&Value> {
        match path {
            Some(path) => {
                if let Value::Object(obj) = self {
                    match obj.get(&path.first) {
                        Some(val) => {
                           let v= val.borrow().get_by_path(path.remainder.as_deref());
                            todo!()
                        }
                        None => None,
                    }
                } else {
                    None
                }
            }
            None => Some(self),
        }
    }
}

impl From<crate::raw::raw_value::RawValue> for Value {
    fn from(value: crate::raw::raw_value::RawValue) -> Self {
        match value {
            crate::raw::raw_value::RawValue::Object(raw_object) => todo!(),
            crate::raw::raw_value::RawValue::Array(raw_array) => Value::array(raw_array),
            crate::raw::raw_value::RawValue::Boolean(b) => Value::Boolean(b),
            crate::raw::raw_value::RawValue::Null => Value::Null,
            crate::raw::raw_value::RawValue::String(raw_string) => {
                Value::string(raw_string.to_string())
            }
            crate::raw::raw_value::RawValue::Number(number) => Value::number(number),
            crate::raw::raw_value::RawValue::Substitution(substitution) => {
                Value::substitution(substitution)
            }
            crate::raw::raw_value::RawValue::Concat(concat) => Value::concat(concat),
            crate::raw::raw_value::RawValue::AddAssign(add_assign) => Value::add_assign(add_assign),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
