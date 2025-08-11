use crate::config_options::ConfigOptions;
use ahash::HashMap;
use derive_more::Constructor;

#[derive(Debug, Clone, Eq, PartialEq, Default, Constructor)]
pub struct ConfigParseOptions {
    pub options: ConfigOptions,
    pub includes: HashMap<String, u8>,
}