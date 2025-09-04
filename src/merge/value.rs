use tracing::trace;

use crate::{
    error::Error,
    merge::{
        add_assign::AddAssign, array::Array, concat::Concat, delay_replacement::DelayReplacement,
        object::Object, path::RefPath, substitution::Substitution,
    },
};
use std::{cell::RefCell, fmt::Display};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Value {
    Object(Object),
    Array(Array),
    Boolean(bool),
    Null,
    #[default]
    None,
    String(String),
    Number(serde_json::Number),
    Substitution(Substitution),
    Concat(Concat),
    AddAssign(AddAssign),
    DelayReplacement(DelayReplacement),
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

    pub(crate) fn delay_replacement<I>(value: I) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        let d = DelayReplacement::from_iter(value);
        Value::DelayReplacement(d.flatten())
    }

    pub(crate) fn ty(&self) -> &'static str {
        match self {
            Value::Object(_) => "object",
            Value::Array(_) => "array",
            Value::Boolean(_) => "boolean",
            Value::Null => "null",
            Value::None => "none",
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Substitution(_) => "substitution",
            Value::Concat(_) => "concat",
            Value::AddAssign(_) => "add_assign",
            Value::DelayReplacement(_) => "delay_replacement",
        }
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        match self {
            Value::Object(object) => object.try_become_merged(),
            Value::Array(array) => array.try_become_merged(),
            Value::Boolean(_) | Value::Null | Value::None | Value::String(_) | Value::Number(_) => {
                true
            }
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayReplacement(_) => false,
        }
    }

    /// Replaces left value to right value if they are simple values. If value contains substitution,
    /// it's impossible to determine the replace behavior, for different type values, right value will override left
    /// value, for add assing(+=)， right value will add to the left value(array), for object vlaues, it will trigger
    /// a object merge operation. If there's any substitution exists in the value, it's impossible for now to determine the
    /// replace behavior, so we construct a new dely replacement value wraps them for future resolve.
    pub(crate) fn replacement(path: &RefPath, left: Value, right: Value) -> crate::Result<Value> {
        trace!("replacement: `{}`: `{}` <- `{}`", path, left, right);
        let new_val = match left {
            Value::Object(mut obj_left) => match right {
                Value::Object(right) => {
                    // Merge two objects
                    obj_left.merge(right, Some(path))?;
                    Value::object(obj_left)
                }
                Value::Array(_)
                | Value::Boolean(_)
                | Value::Null
                | Value::None
                | Value::String(_)
                | Value::Number(_) => right,
                Value::Substitution(_) => {
                    let left = Value::object(obj_left);
                    Value::delay_replacement([left, right])
                }
                Value::Concat(concat) => {
                    let try_resolved = concat.try_resolve(path)?;
                    match try_resolved {
                        Value::Object(object) => {
                            obj_left.merge(object, Some(path))?;
                            Value::object(obj_left)
                        }
                        Value::Concat(mut concat) => {
                            let left = Value::object(obj_left);
                            concat.push_front(RefCell::new(left), None);
                            Value::concat(concat)
                        }
                        // Concat result must not be Substitution DelayReplacement
                        other => other,
                    }
                }
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "object",
                        right_type: right.ty(),
                    });
                }
                Value::DelayReplacement(mut delay_merge) => {
                    let left = Value::object(obj_left);
                    delay_merge.push_front(RefCell::new(left));
                    Value::DelayReplacement(delay_merge)
                }
            },
            Value::Array(mut array_left) => match right {
                Value::Substitution(_) | Value::DelayReplacement(_) => {
                    Value::delay_replacement([Value::array(array_left), right])
                }
                Value::Concat(concat) => {
                    let right = concat.try_resolve(path)?;
                    match right {
                        Value::Array(array) => {
                            let left = Value::Array(array_left);
                            let right = Value::Array(array);
                            Self::concatenate(path, left, None, right)?
                        }
                        Value::Concat(concat) => {
                            let left = Value::Array(array_left);
                            let right = Value::Concat(concat);
                            Value::delay_replacement([left, right])
                        }
                        right => right,
                    }
                }
                Value::AddAssign(add_assign) => {
                    let inner: Value = add_assign.into();
                    let unmerged = inner.is_unmerged();
                    array_left.push(RefCell::new(inner));
                    if unmerged {
                        array_left.as_unmerged()
                    }
                    Value::array(array_left)
                }
                right => right,
            },
            Value::Null => match right {
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "null",
                        right_type: right.ty(),
                    });
                }
                other => other,
            },
            // expand the first add assign to array
            Value::None => match right {
                Value::AddAssign(add_assign) => {
                    let value = add_assign.try_resolve(path)?;
                    let array = if value.is_merged() {
                        Array::Merged(vec![RefCell::new(value)])
                    } else {
                        Array::Unmerged(vec![RefCell::new(value)])
                    };
                    Value::Array(array)
                }
                right => right,
            },
            Value::Boolean(_) | Value::String(_) | Value::Number(_) => match right {
                // The substitution expression and concat might refer to the previous value
                // so we cannot replace it directly
                Value::Substitution(_) => Value::delay_replacement([left, right]),
                Value::Concat(concat) => {
                    // try resolve the concat at this time if it not contains substitution
                    let right = concat.try_resolve(path)?;
                    match right {
                        Value::Concat(_) => Value::delay_replacement([left, right]),
                        Value::AddAssign(_) => {
                            return Err(Error::ConcatenateDifferentType {
                                path: path.to_string(),
                                left_type: left.ty(),
                                right_type: "add_assign",
                            });
                        }
                        other => other,
                    }
                }
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: left.ty(),
                        right_type: right.ty(),
                    });
                }
                other => other,
            },
            // left value is impossible to be an add assign, because when merging two objects, the first add assign
            // value will always expand to an array.
            Value::AddAssign(_) => {
                return Err(Error::ConcatenateDifferentType {
                    path: path.to_string(),
                    left_type: left.ty(),
                    right_type: right.ty(),
                });
            }
            Value::Substitution(_) | Value::Concat(_) | Value::DelayReplacement(_) => {
                Value::delay_replacement([left, right])
            }
        };
        trace!("replacement result: `{path}`=`{new_val}`");
        Ok(new_val)
    }

    pub(crate) fn concatenate(
        path: &RefPath,
        left: Value,
        space: Option<String>,
        right: Value,
    ) -> crate::Result<Value> {
        trace!("concatenate: `{}`: `{}` <- `{}`", path, left, right);
        let val = match left {
            Value::Object(mut left_obj) => match right {
                Value::None => Value::object(left_obj),
                Value::Object(right_obj) => {
                    left_obj.merge(right_obj, Some(path))?;
                    Value::object(left_obj)
                }
                Value::Null
                | Value::Array(_)
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Number(_)
                | Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "object",
                        right_type: right.ty(),
                    });
                }
                Value::Substitution(_) => {
                    let left = Value::object(left_obj);
                    Value::concat(Concat::two(left, space, right))
                }
                Value::Concat(mut concat) => {
                    let left = Value::object(left_obj);
                    concat.push_front(RefCell::new(left), space);
                    Value::concat(concat)
                }
                Value::DelayReplacement(_) => {
                    let left = Value::object(left_obj);
                    Value::concat(Concat::two(left, space, right))
                }
            },
            Value::Array(mut left_array) => {
                if let Value::Array(right_array) = right {
                    left_array.extend(right_array.into_inner());
                    Value::array(left_array)
                } else {
                    return Err(crate::error::Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "array",
                        right_type: right.ty(),
                    });
                }
            }
            Value::None => match space {
                Some(space) => match right {
                    Value::Null | Value::Boolean(_) | Value::String(_) | Value::Number(_) => {
                        Value::string(format!("{space}{right}"))
                    }
                    Value::None => Value::string(space),
                    Value::Substitution(_) => Value::concat(Concat::two(left, Some(space), right)),
                    right => right,
                },
                _ => right,
            },
            Value::Null | Value::Boolean(_) | Value::String(_) | Value::Number(_) => match right {
                Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {
                    match space {
                        Some(space) => Value::string(format!("{left}{space}{right}")),
                        None => Value::string(format!("{left}{right}")),
                    }
                }
                Value::None => match space {
                    Some(space) => Value::string(format!("{left}{space}")),
                    None => Value::string(left.to_string()),
                },
                Value::Substitution(_) => Value::concat(Concat::two(left, space, right)),
                _ => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: left.ty(),
                        right_type: right.ty(),
                    });
                }
            },
            Value::Substitution(_) => Value::concat(Concat::two(left, space, right)),
            Value::Concat(mut concat) => {
                concat.push_back(space, RefCell::new(right));
                Value::concat(concat)
            }
            Value::AddAssign(_) => {
                return Err(Error::ConcatenateDifferentType {
                    path: path.to_string(),
                    left_type: left.ty(),
                    right_type: right.ty(),
                });
            }
            Value::DelayReplacement(_) => Value::concat(Concat::two(left, space, right)),
        };
        trace!("concatenate result: `{path}`=`{val}`");
        debug_assert!(!matches!(
            val,
            Value::DelayReplacement(_) | Value::Substitution(_) | Value::AddAssign(_)
        ));
        Ok(val)
    }

    pub(crate) fn is_merged(&self) -> bool {
        match self {
            Value::Object(object) => object.is_merged(),
            Value::Array(array) => array.is_merged(),
            Value::Boolean(_) | Value::String(_) | Value::Number(_) | Value::Null | Value::None => {
                true
            }
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayReplacement(_) => false,
        }
    }

    pub(crate) fn is_unmerged(&self) -> bool {
        !self.is_merged()
    }

    pub(crate) fn resolve_add_assign(&mut self) {
        if let Value::Object(object) = self {
            object.resolve_add_assign();
        } else if let Value::AddAssign(add_assign) = self {
            let val = std::mem::take(&mut add_assign.0);
            *self = Value::Array(Array::new(vec![RefCell::new(*val)]));
            self.try_become_merged();
        }
    }

    pub(crate) fn resolve(&mut self) -> crate::Result<()> {
        if let Value::Object(object) = self {
            object.substitute()?;
        }
        self.resolve_add_assign();
        self.try_become_merged();
        Ok(())
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::raw_value::RawValue,
    ) -> crate::Result<Self> {
        let mut value = match raw {
            crate::raw::raw_value::RawValue::Object(raw_object) => {
                let object = Object::from_raw(parent, raw_object)?;
                Value::object(object)
            }
            crate::raw::raw_value::RawValue::Array(raw_array) => {
                let array = Array::from_raw(parent, raw_array)?;
                Value::array(array)
            }
            crate::raw::raw_value::RawValue::Boolean(b) => Value::Boolean(b),
            crate::raw::raw_value::RawValue::Null => Value::Null,
            crate::raw::raw_value::RawValue::String(raw_string) => {
                Value::string(raw_string.to_string())
            }
            crate::raw::raw_value::RawValue::Number(number) => Value::number(number),
            crate::raw::raw_value::RawValue::Substitution(substitution) => {
                Value::substitution(substitution)
            }
            crate::raw::raw_value::RawValue::Concat(concat) => {
                let concat = Concat::from_raw(parent, concat)?;
                Value::concat(concat)
            }
            crate::raw::raw_value::RawValue::AddAssign(add_assign) => {
                let add_assign = AddAssign::from_raw(parent, add_assign)?;
                Value::add_assign(add_assign)
            }
        };
        value.try_become_merged();
        Ok(value)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Object(object) => write!(f, "{object}"),
            Value::Array(array) => write!(f, "{array}"),
            Value::Boolean(boolean) => write!(f, "{boolean}"),
            Value::None => write!(f, "none"),
            Value::Null => write!(f, "null"),
            Value::String(string) => write!(f, "{string}"),
            Value::Number(number) => write!(f, "{number}"),
            Value::Substitution(substitution) => write!(f, "{substitution}"),
            Value::Concat(concat) => write!(f, "{concat}"),
            Value::AddAssign(add_assign) => write!(f, "{add_assign}"),
            Value::DelayReplacement(delay_merge) => write!(f, "{delay_merge}"),
        }
    }
}
