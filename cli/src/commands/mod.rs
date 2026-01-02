//! CLI command handlers
//!
//! Each subcommand is implemented in its own module for maintainability.

pub mod chunk;
pub mod diff;
pub mod embed;
pub mod impact;
pub mod index;
pub mod info;
pub mod init;
pub mod map;
pub mod pack;
pub mod scan;

// Re-export command functions for main.rs
pub use chunk::cmd_chunk;
pub use diff::cmd_diff;
pub use embed::{cmd_embed, EmbedConfig, EmbedOutputFormat};
pub use impact::cmd_impact;
pub use index::cmd_index;
pub use info::cmd_info;
pub use init::{cmd_init, ConfigFormat, ConfigTemplate};
pub use map::cmd_map;
pub use pack::cmd_pack;
pub use scan::cmd_scan;
