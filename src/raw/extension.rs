use crate::raw::field::ObjectField;
use crate::raw::include::Inclusion;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;

pub trait IntRawValueExt {
    fn r(self) -> RawValue;
}

impl IntRawValueExt for i32 {
    fn r(self) -> RawValue {
        RawValue::Int(self as i64)
    }
}

impl IntRawValueExt for i64 {
    fn r(self) -> RawValue {
        RawValue::Int(self)
    }
}

pub trait FloatRawValueExt {
    fn r(self) -> RawValue;
}

impl FloatRawValueExt for f32 {
    fn r(self) -> RawValue {
        RawValue::Float(self as f64)
    }
}

impl FloatRawValueExt for f64 {
    fn r(self) -> RawValue {
        RawValue::Float(self)
    }
}

pub trait BoolRawValueExt {
    fn r(self) -> RawValue;
}

impl BoolRawValueExt for bool {
    fn r(self) -> RawValue {
        RawValue::Boolean(self)
    }
}

pub trait StringRawValueExt {
    fn r(self) -> RawValue;
}

impl<T: Into<String>> StringRawValueExt for T {
    fn r(self) -> RawValue {
        RawValue::String(RawString::QuotedString(self.into()))
    }
}

pub trait ObjectRawValueExt {
    fn r(self) -> RawValue;
}

impl ObjectRawValueExt for RawObject {
    fn r(self) -> RawValue {
        RawValue::Object(self)
    }
}

// impl ObjectRawValueExt for (String, RawValue) {
//     fn r(self) -> RawValue {
//         RawValue::Object(RawObject::new(vec![self.f()]))
//     }
// }

// impl ObjectRawValueExt for (&str, RawValue) {
//     fn r(self) -> RawValue {
//         RawValue::Object(RawObject::new(vec![self.f()]))
//     }
// }

// impl ObjectRawValueExt for Vec<(String, RawValue)> {
//     fn r(self) -> RawValue {
//         let fields = self.into_iter().map(|v| v.f());
//         RawValue::Object(RawObject::new(fields.collect()))
//     }
// }

// impl ObjectRawValueExt for Vec<(&str, RawValue)> {
//     fn r(self) -> RawValue {
//         let fields = self.into_iter().map(|v| v.f());
//         RawValue::Object(RawObject::new(fields.collect()))
//     }
// }

pub trait ArrayRawValueExt {
    fn r(self) -> RawValue;
}

impl<T: IntoIterator<Item=RawValue>> ArrayRawValueExt for T {
    fn r(self) -> RawValue {
        RawValue::Array(RawArray::new(self.into_iter().collect()))
    }
}

pub trait ObjectFieldKVExt {
    fn f(self) -> ObjectField;
}

// impl ObjectFieldKVExt for (String, RawValue) {
//     fn f(self) -> ObjectField {
//         ObjectField::KeyValue(self.0, self.1)
//     }
// }
// 
// impl ObjectFieldKVExt for (&str, RawValue) {
//     fn f(self) -> ObjectField {
//         ObjectField::KeyValue(self.0.to_string(), self.1)
//     }
// }

pub trait ObjectFieldIncludeExt {
    fn f(self) -> ObjectField;
}

impl ObjectFieldIncludeExt for Inclusion {
    fn f(self) -> ObjectField {
        ObjectField::Inclusion(self)
    }
}