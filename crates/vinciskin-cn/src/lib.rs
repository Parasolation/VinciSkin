//! Install discovery and catalog alignment for the Chinese League client.

pub mod catalog;
pub mod error;
pub mod install;
pub mod version;

pub use error::CnError;
pub use version::PatchVersion;
