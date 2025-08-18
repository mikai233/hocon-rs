pub mod config;
mod config_options;
pub mod error;
pub mod factory;
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
    mod add_assign;
    mod array;
    mod concat;
    mod delay_merge;
    mod inclusion;
    mod object;
    mod substitution;
    mod vlaue;
}

pub type Result<T> = std::result::Result<T, error::Error>;
