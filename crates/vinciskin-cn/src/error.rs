//! The error type every fallible call in this crate returns.

use thiserror::Error;

/// Every way a discovery or catalog call can fail.
#[derive(Debug, Error)]
pub enum CnError {
    /// A version string did not parse.
    #[error("invalid patch version: {0}")]
    Version(String),

    /// A registry key could not be read.
    #[error("registry read failed: {0}")]
    Registry(String),

    /// A catalog body did not match its expected shape.
    #[error("catalog parse failed: {0}")]
    CatalogParse(String),

    /// A catalog's patchline does not match Riot's.
    #[error("catalog out of sync: riot {riot}, tencent {tencent}")]
    OutOfSync {
        /// Riot's Data Dragon version.
        riot: String,
        /// The Tencent table's version.
        tencent: String,
    },

    /// A network call failed.
    #[error("network failed: {0}")]
    Network(String),

    /// A file or directory call failed.
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_error_names_the_input() {
        let error = CnError::Version("16.x".into());
        assert_eq!(error.to_string(), "invalid patch version: 16.x");
    }

    #[test]
    fn an_out_of_sync_error_names_both_versions() {
        let error = CnError::OutOfSync {
            riot: "16.19.1".into(),
            tencent: "16.20.0".into(),
        };
        assert_eq!(
            error.to_string(),
            "catalog out of sync: riot 16.19.1, tencent 16.20.0"
        );
    }
}
