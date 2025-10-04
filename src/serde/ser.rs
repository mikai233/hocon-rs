use crate::Result;
use serde::ser::Serialize;
use std::io;

#[inline]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ?Sized + Serialize,
{
    serde_json::to_writer(writer, value)?;
    Ok(())
}

#[inline]
pub fn to_writer_pretty<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ?Sized + Serialize,
{
    serde_json::to_writer_pretty(writer, value)?;
    Ok(())
}

#[inline]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    let data = serde_json::to_vec(value)?;
    Ok(data)
}

#[inline]
pub fn to_vec_pretty<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    let data = serde_json::to_vec_pretty(value)?;
    Ok(data)
}

#[inline]
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let string = serde_json::to_string(value)?;
    Ok(string)
}

#[inline]
pub fn to_string_pretty<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let string = serde_json::to_string_pretty(value)?;
    Ok(string)
}

#[cfg(test)]
mod tests {
    use crate::{Result, Value, serde::ser};
    #[test]
    fn test_serialization() -> Result<()> {
        let value = Value::array_from_iter([
            Value::Number(1.into()),
            Value::Number(2.into()),
            Value::Number(3.into()),
        ]);
        let mut w = vec![];
        ser::to_writer(&mut w, &value)?;
        ser::to_writer_pretty(&mut w, &value)?;
        ser::to_vec(&value)?;
        ser::to_vec_pretty(&value)?;
        ser::to_string(&value)?;
        ser::to_string_pretty(&value)?;
        Ok(())
    }
}
