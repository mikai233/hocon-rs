use crate::object::Object;
use crate::value::Value;

pub trait IntValueExt {
    fn v(self) -> Value;
}

impl IntValueExt for i32 {
    fn v(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntValueExt for i64 {
    fn v(self) -> Value {
        Value::Int(self)
    }
}

pub trait FloatValueExt {
    fn v(self) -> Value;
}

impl FloatValueExt for f32 {
    fn v(self) -> Value {
        Value::Float(self as f64)
    }
}

impl FloatValueExt for f64 {
    fn v(self) -> Value {
        Value::Float(self)
    }
}

pub trait BoolValueExt {
    fn v(self) -> Value;
}

impl BoolValueExt for bool {
    fn v(self) -> Value {
        Value::Boolean(self)
    }
}

pub trait StringValueExt {
    fn v(self) -> Value;
}

impl<T: Into<String>> StringValueExt for T {
    fn v(self) -> Value {
        Value::String(self.into())
    }
}

pub trait ObjectValueExt {
    fn v(self) -> Value;
}

impl ObjectValueExt for Object {
    fn v(self) -> Value {
        Value::Object(self)
    }
}

pub trait ArrayValueExt {
    fn v(self) -> Value;
}

impl<T: IntoIterator<Item=Value>> ArrayValueExt for T {
    fn v(self) -> Value {
        Value::Array(self.into_iter().collect())
    }
}