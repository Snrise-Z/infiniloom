//! Library module for test project

pub mod utils;

/// A configuration struct
pub struct Config {
    pub debug: bool,
    pub max_connections: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debug: false,
            max_connections: 100,
        }
    }
}

/// Initialize the library
pub fn init() -> Config {
    Config::default()
}
