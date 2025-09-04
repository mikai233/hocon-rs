use ::serde::{Serialize, de::DeserializeOwned};

pub mod config;
mod config_options;
pub mod error;
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
pub use config::Config;
pub use config_options::ConfigOptions;
pub use value::Value;

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
    if let Some(v) = iter.next() {
        write!(f, "{v}")?;
        for v in iter {
            write!(f, "{sep}")?;
            write!(f, "{v}")?;
        }
    }
    Ok(())
}

#[inline]
pub(crate) fn join_format<I, V, S, R>(
    mut iter: I,
    f: &mut std::fmt::Formatter<'_>,
    separator_formatter: S,
    value_formatter: R,
) -> std::fmt::Result
where
    I: Iterator<Item = V>,
    S: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
    R: Fn(&mut std::fmt::Formatter, V) -> std::fmt::Result,
{
    if let Some(v) = iter.next() {
        value_formatter(f, v)?;
        for v in iter {
            separator_formatter(f)?;
            value_formatter(f, v)?;
        }
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
    if let Some(v) = iter.next() {
        write!(f, "{v:?}")?;
        for v in iter {
            write!(f, "{sep}")?;
            write!(f, "{v:?}")?;
        }
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
