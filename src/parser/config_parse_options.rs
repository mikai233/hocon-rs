use crate::config_options::ConfigOptions;
use derive_more::Constructor;

#[derive(Debug, Clone, Eq, PartialEq, Default, Constructor)]
pub struct ConfigParseOptions {
    pub options: ConfigOptions,
    pub includes: Vec<String>,
}

impl Into<ConfigParseOptions> for ConfigOptions {
    fn into(self) -> ConfigParseOptions {
        ConfigParseOptions {
            options: self,
            includes: Default::default(),
        }
    }
}
