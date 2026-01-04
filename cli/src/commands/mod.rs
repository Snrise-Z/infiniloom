//! CLI command handlers
//!
//! Each subcommand is implemented in its own module for maintainability.

pub(crate) mod chunk;
pub(crate) mod diff;
pub(crate) mod embed;
pub(crate) mod impact;
pub(crate) mod index;
pub(crate) mod info;
pub(crate) mod init;
pub(crate) mod map;
pub(crate) mod pack;
pub(crate) mod scan;

// Re-export command functions for main.rs
pub(crate) use chunk::cmd_chunk;
pub(crate) use diff::cmd_diff;
pub(crate) use embed::{cmd_embed, EmbedConfig, EmbedOutputFormat};
pub(crate) use impact::cmd_impact;
pub(crate) use index::cmd_index;
pub(crate) use info::cmd_info;
pub(crate) use init::{cmd_init, ConfigFormat, ConfigTemplate};
pub(crate) use map::cmd_map;
pub(crate) use pack::cmd_pack;
pub(crate) use scan::cmd_scan;
