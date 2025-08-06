use crate::error::Error;
use crate::value::Value;

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value as i64)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl TryInto<i32> for Value {
    type Error = Error;

    fn try_into(self) -> Result<i32, Self::Error> {
        match self {
            Value::Int(int) => {
                match i32::try_from(int) {
                    Ok(int) => Ok(int),
                    Err(_) => Err(Error::PrecisionLoss {
                        from: self.ty(),
                        to: "i32",
                    })
                }
            }
            value => Err(Error::InvalidConversion {
                from: value.ty(),
                to: "i32",
            })
        }
    }
}