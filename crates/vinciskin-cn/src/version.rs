//! The patch version every other module compares against.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::CnError;

/// A League patch version. Revision is carried but never compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatchVersion {
    /// The season.
    pub major: u32,
    /// The patch within a season.
    pub minor: u32,
    /// The revision, which [`PatchVersion::same_patch`] ignores.
    pub revision: u32,
}

impl PatchVersion {
    /// `15.20.1` or `15.20`, with a missing revision read as zero.
    pub fn parse(text: &str) -> Result<PatchVersion, CnError> {
        let mut parts = text.trim().split('.');

        let major = component(parts.next(), text)?;
        let minor = component(parts.next(), text)?;
        let revision = match parts.next() {
            Some(part) => component(Some(part), text)?,
            None => 0,
        };

        if parts.next().is_some() {
            return Err(CnError::Version(text.to_string()));
        }

        Ok(PatchVersion {
            major,
            minor,
            revision,
        })
    }

    /// Whether two versions name one patchline.
    pub fn same_patch(&self, other: &PatchVersion) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl fmt::Display for PatchVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.revision)
    }
}

fn component(part: Option<&str>, text: &str) -> Result<u32, CnError> {
    let part = part.ok_or_else(|| CnError::Version(text.to_string()))?;

    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CnError::Version(text.to_string()));
    }

    part.parse::<u32>()
        .map_err(|_| CnError::Version(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_version_parses() {
        let version = PatchVersion::parse("15.20.1").unwrap();
        assert_eq!(
            (version.major, version.minor, version.revision),
            (15, 20, 1)
        );
    }

    #[test]
    fn a_missing_revision_reads_as_zero() {
        let version = PatchVersion::parse("16.19").unwrap();
        assert_eq!(
            (version.major, version.minor, version.revision),
            (16, 19, 0)
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        assert_eq!(PatchVersion::parse("  16.19.2 ").unwrap().revision, 2);
    }

    #[test]
    fn garbage_is_rejected() {
        for text in [
            "",
            "16",
            "16.x",
            "x.19",
            "16.19.1.2",
            "16..1",
            "16.19.",
            "-1.2",
            "16.19.-1",
        ] {
            assert!(
                matches!(PatchVersion::parse(text), Err(CnError::Version(_))),
                "{text:?} should not parse"
            );
        }
    }

    #[test]
    fn same_patch_reads_major_and_minor_only() {
        let base = PatchVersion::parse("16.19.1").unwrap();
        assert!(base.same_patch(&PatchVersion::parse("16.19.9").unwrap()));
        assert!(base.same_patch(&PatchVersion::parse("16.19").unwrap()));
        assert!(!base.same_patch(&PatchVersion::parse("16.20.1").unwrap()));
        assert!(!base.same_patch(&PatchVersion::parse("17.19.1").unwrap()));
    }

    #[test]
    fn display_spells_all_three_components() {
        assert_eq!(
            PatchVersion::parse("16.19.1").unwrap().to_string(),
            "16.19.1"
        );
        assert_eq!(PatchVersion::parse("16.19").unwrap().to_string(), "16.19.0");
    }
}
