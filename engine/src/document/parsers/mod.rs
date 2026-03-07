//! Format-specific document parsers.

pub mod csv;
pub mod docx;
pub mod html;
pub mod markdown;
pub mod plaintext;

#[cfg(feature = "document-xlsx")]
pub mod xlsx;
