use crate::merge::{
    add_assign::AddAssign, array::Array, concat::Concat, delay_merge::DelayMerge, object::Object,
    substitution::Substitution,
};
use log::debug;
use std::{cell::RefCell, fmt::Display};

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

    pub(crate) fn as_delay_merge_mut(&mut self) -> &mut DelayMerge {
        if let Value::DelayMerge(delay_merge) = self {
            return delay_merge;
        } else {
            panic!("value should be DelayMerge")
        }
    }

    pub(crate) fn as_concat_mut(&mut self) -> &mut Concat {
        if let Value::Concat(concat) = self {
            return concat;
        } else {
            panic!("value should be Concat")
        }
    }

    pub(crate) fn as_add_assign_mut(&mut self) -> &mut AddAssign {
        if let Value::AddAssign(add_assign) = self {
            return add_assign;
        } else {
            panic!("value should be Concat")
        }
    }

    pub(crate) fn as_array_mut(&mut self) -> &mut Array {
        if let Value::Array(array) = self {
            return array;
        } else {
            panic!("value should be array")
        }
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        match self {
            Value::Object(object) => object.try_become_merged(),
            Value::Array(array) => array.iter_mut().all(|v| v.get_mut().try_become_merged()),
            Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => true,
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayMerge(_) => false,
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
                        .all(|v| matches!(&*v.borrow(), Value::Object(_) | Value::Substitution(_)))
                    {
                        // the concat result maybe an object, so we need to push the left object for potential
                        // object concat
                        let left = Value::object(obj_left);
                        concat.push_front(RefCell::new(left));
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
                    delay_merge.push_front(RefCell::new(left));
                    Value::DelayMerge(delay_merge)
                }
            },
            Value::Array(mut array) => {
                if let Value::AddAssign(add_assign) = right {
                    array.push(RefCell::new(add_assign.into()));
                    Value::array(array)
                } else {
                    right
                }
            }
            Value::Boolean(_)
            | Value::Null
            | Value::String(_)
            | Value::Number(_)
            | Value::AddAssign(_) => match right {
                Value::Substitution(_) => {
                    Value::delay_merge(vec![left, right])
                }
                other => other,
            },
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
        debug!("concatenate: {} <- {}", left, right);
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
                    // Value::delay_merge(vec![left, right])
                    Value::concat(Concat::from_iter(vec![left, right]))
                }
                Value::Concat(mut concat) => {
                    let left = Value::object(left_obj);
                    concat.push_front(RefCell::new(left));
                    Value::concat(concat)
                    // let right = concat.reslove()?;
                    // Self::concatenate(left, right)?
                    // return Err(crate::error::Error::ConcatenationDifferentType {
                    //     ty1: "object",
                    //     ty2: right.ty(),
                    // });
                }
                Value::AddAssign(_) => {
                    return Err(crate::error::Error::ConcatenationDifferentType {
                        ty1: "object",
                        ty2: right.ty(),
                    });
                }
                Value::DelayMerge(_) => {
                    let left = Value::object(left_obj);
                    // Value::delay_merge(vec![left, right])
                    Value::concat(Concat::from_iter(vec![left, right]))
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
            Value::Substitution(_) => {
                // Value::delay_merge(vec![left, right]),
                Value::concat(Concat::from_iter(vec![left, right]))
            }
            Value::Concat(mut concat) => {
                // let left = concat.reslove()?;
                // Self::concatenate(left, right)?
                concat.push_back(RefCell::new(right));
                // println!("left:{left} right:{right}");
                // return Err(crate::error::Error::ConcatenationDifferentType {
                //     ty1: left.ty(),
                //     ty2: right.ty(),
                // });
                Value::concat(concat)
            }
            Value::AddAssign(_) => {
                return Err(crate::error::Error::ConcatenationDifferentType {
                    ty1: left.ty(),
                    ty2: right.ty(),
                });
            }
            Value::DelayMerge(_) => {
                // Value::delay_merge(vec![left, right]),
                Value::concat(Concat::from_iter(vec![left, right]))
            }
        };
        debug!("concatenate result: {val}");
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

    pub(crate) fn substitute(root: &Object, value: &RefCell<Value>) -> crate::Result<()> {
        let borrowed = value.borrow();
        match &*borrowed {
            Value::Object(object) => {
                if object.is_unmerged() {
                    for val in object.values() {
                        Self::substitute(root, val)?;
                    }
                }
                drop(borrowed);
                value.borrow_mut().try_become_merged();
            }
            Value::Array(array) => {
                for ele in array.iter() {
                    Self::substitute(root, ele)?;
                }
                drop(borrowed);
                // value.borrow_mut().try_become_merged();
            }
            Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
            Value::Substitution(substitution) => {
                // TODO maybe we should remove it first to avoid cycle substitue?
                debug!("substitute: {}", substitution);
                root.invoke_on_target_path(&substitution.path, |target| {
                    debug!("find substitution: {} of {}", target.borrow(), substitution.path);
                    if target.borrow().is_merged() {
                        *value.borrow_mut() = target.borrow().clone();
                    } else {
                        Self::substitute(root, target)?;
                    }
                    Ok(())
                })?;
                // TODO fetch from env
            }
            Value::Concat(_) => {
                drop(borrowed);
                let mut current: Option<Value> = None;
                loop {
                    let v = {
                        let mut borrowed = value.borrow_mut();
                        let concat = borrowed.as_concat_mut();
                        debug!("substitute pop concat {}", concat);
                        concat.pop_back()
                    };
                    match v {
                        None => {
                            break;
                        }
                        Some(v) => {
                            if !v.borrow().is_merged() {
                                Self::substitute(root, &v)?;
                            }
                            match current {
                                None => {
                                    current = Some(v.into_inner());
                                }
                                Some(c) => {
                                    let n = Value::concatenate(v.into_inner(), c)?;
                                    current = Some(n);
                                }
                            }
                        }
                    }
                }
                match current {
                    None => {
                        *value.borrow_mut() = Value::Null;
                    }
                    Some(mut c) => {
                        c.try_become_merged();
                        *value.borrow_mut() = c;
                    }
                }
            }
            Value::AddAssign(_) => {
                drop(borrowed);
                let add_assign = std::mem::take(value.borrow_mut().as_add_assign_mut());
                let v: RefCell<Value> = RefCell::new(add_assign.into());
                Self::substitute(root, &v)?;
                let mut v = v.into_inner();
                v.try_become_merged();
                let add_assign = AddAssign::new(Box::new(v));
                *value.borrow_mut() = Value::add_assign(add_assign);
            }
            Value::DelayMerge(_) => {
                drop(borrowed);
                let mut current: Option<Value> = None;
                loop {
                    let v = value.borrow_mut().as_delay_merge_mut().pop_back();
                    match v {
                        None => {
                            break;
                        }
                        Some(v) => {
                            if !v.borrow().is_merged() {
                                Self::substitute(root, &v)?;
                            }
                            match current {
                                Some(c) => {
                                    let n = Value::replacement(v.into_inner(), c)?;
                                    current = Some(n);
                                }
                                None => {
                                    current = Some(v.into_inner());
                                }
                            }
                        }
                    }
                }
                match current {
                    Some(mut c) => {
                        c.try_become_merged();
                        *value.borrow_mut() = c;
                    }
                    None => {
                        *value.borrow_mut() = Value::Null;
                    }
                }
            }
        }
        Ok(())
    }
}

impl TryFrom<crate::raw::raw_value::RawValue> for Value {
    type Error = crate::error::Error;

    fn try_from(value: crate::raw::raw_value::RawValue) -> Result<Self, Self::Error> {
        let value = match value {
            crate::raw::raw_value::RawValue::Object(raw_object) => {
                let object: Object = raw_object.try_into()?;
                Value::object(object)
            }
            crate::raw::raw_value::RawValue::Array(raw_array) => {
                let array: Array = raw_array.try_into()?;
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
                let concat: Concat = concat.try_into()?;
                Value::concat(concat)
            }
            crate::raw::raw_value::RawValue::AddAssign(add_assign) => {
                let add_assign: AddAssign = add_assign.try_into()?;
                Value::add_assign(add_assign)
            }
        };
        Ok(value)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Object(object) => write!(f, "{object}"),
            Value::Array(array) => write!(f, "{array}"),
            Value::Boolean(boolean) => write!(f, "{boolean}"),
            Value::Null => write!(f, "null"),
            Value::String(string) => write!(f, "{string}"),
            Value::Number(number) => write!(f, "{number}"),
            Value::Substitution(substitution) => write!(f, "{substitution}"),
            Value::Concat(concat) => write!(f, "{concat}"),
            Value::AddAssign(add_assign) => write!(f, "{add_assign}"),
            Value::DelayMerge(delay_merge) => write!(f, "{delay_merge}"),
        }
    }
}
