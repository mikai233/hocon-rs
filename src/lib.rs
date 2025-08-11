extern crate core;

pub mod value;
pub mod config;
pub mod error;
pub mod transform;
pub mod object;
pub mod syntax;
pub mod extension;
pub mod raw;
pub mod factory;
pub(crate) mod path;
pub mod parser;
mod key;
mod config_options;

pub type Result<T> = std::result::Result<T, error::Error>;