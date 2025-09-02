extern crate core;

use ::serde::{Serialize, de::DeserializeOwned};

use crate::value::Value;

pub mod config;
mod config_options;
pub mod error;
pub mod macros;
pub mod object;
pub mod parser;
pub(crate) mod path;
pub mod raw;
pub mod serde;
pub mod syntax;
pub mod transform;
pub mod value;
mod merge {
    pub(crate) mod add_assign;
    pub(crate) mod array;
    pub(crate) mod concat;
    pub(crate) mod delay_replacement;
    pub(crate) mod object;
    pub(crate) mod path;
    pub(crate) mod substitution;
    pub(crate) mod value;
}

pub type Result<T> = std::result::Result<T, error::Error>;

pub fn to_value<T>(value: T) -> crate::Result<Value>
where
    T: Serialize,
{
    let value: Value = serde_json::to_value(value)?.into();
    Ok(value)
}

pub fn from_value<T>(value: Value) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    T::deserialize(value)
}

#[inline]
pub(crate) fn join<I, V>(
    mut iter: I,
    sep: &str,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result
where
    I: Iterator<Item = V>,
    V: std::fmt::Display,
{
    match iter.next() {
        Some(v) => {
            write!(f, "{v}")?;
            for v in iter {
                write!(f, "{sep}")?;
                write!(f, "{v}")?;
            }
        }
        None => {}
    }
    Ok(())
}

#[inline]
pub(crate) fn join_debug<I, V>(
    mut iter: I,
    sep: &str,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result
where
    I: Iterator<Item = V>,
    V: std::fmt::Debug,
{
    match iter.next() {
        Some(v) => {
            write!(f, "{v:?}")?;
            for v in iter {
                write!(f, "{sep}")?;
                write!(f, "{v:?}")?;
            }
        }
        None => {}
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::fmt::time::LocalTime;

    #[ctor::ctor]
    fn init_tracing() {
        tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .with_max_level(LevelFilter::TRACE)
            .with_timer(LocalTime::rfc_3339())
            .try_init();
    }
}
