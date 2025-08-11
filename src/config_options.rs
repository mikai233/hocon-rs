use crate::syntax::Syntax;
use derive_more::Constructor;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Constructor)]
pub struct ConfigOptions {
    pub syntax: Option<Syntax>,
    pub max_include_depth: u8,
    pub use_system_environment: bool,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            syntax: None,
            max_include_depth: 50,
            use_system_environment: true,
        }
    }
}