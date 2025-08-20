use ::serde::de::DeserializeOwned;

use crate::value::Value;

pub mod config;
mod config_options;
pub mod error;
mod key;
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
    pub(crate) mod inclusion;
    pub(crate) mod object;
    pub(crate) mod substitution;
    pub(crate) mod value;
}

pub type Result<T> = std::result::Result<T, error::Error>;

// pub fn to_value<T>(value: T) -> crate::Result<Value>
// where
//     T: Serialize,
// {
//     value.serialize(serializer)
// }

pub fn from_value<T>(value: Value) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    T::deserialize(value)
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
