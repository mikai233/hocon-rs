use crate::config_options::ConfigOptions;
use derive_more::Constructor;

pub(crate) const MAX_DEPTH: u32 = 50;

#[derive(Debug, Clone, Eq, PartialEq, Default, Constructor)]
pub(crate) struct ConfigParseOptions {
    pub(crate) options: ConfigOptions,
    pub(crate) includes: Vec<String>,
    pub(crate) current_depth: u32,
}

impl Into<ConfigParseOptions> for ConfigOptions {
    fn into(self) -> ConfigParseOptions {
        ConfigParseOptions {
            options: self,
            includes: Default::default(),
            current_depth: 0,
        }
    }
}
