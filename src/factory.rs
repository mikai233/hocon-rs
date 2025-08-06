use crate::config::Config;

pub struct ConfigFactory;

impl ConfigFactory {
    pub fn load() -> Config {
        todo!()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConfigOptions {
    pub include_depth: u8,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            include_depth: 50,
        }
    }
}