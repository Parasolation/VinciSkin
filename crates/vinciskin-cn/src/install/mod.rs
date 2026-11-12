//! Sources of installed League patchlines.

pub mod riot;
pub mod tencent;

use std::path::{Path, PathBuf};

/// One installed League patchline, as a source reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPatchline {
    /// `live`, `pbe`, `tencent`, and the rest.
    pub id: String,
    /// The install root, the folder that holds `Game`.
    pub root: PathBuf,
}

/// A source of installed League patchlines.
pub trait InstallSource {
    /// Every patchline this source can see, empty when nothing is found.
    fn discover(&self) -> Vec<InstalledPatchline>;
}

/// Whether two paths name one install.
///
/// The `\\?\` prefix, the slash kind, a trailing separator and the case are set
/// aside, and case folding is Unicode-aware because a CN root holds Chinese
/// characters.
pub fn same_install(a: &Path, b: &Path) -> bool {
    install_key(a) == install_key(b)
}

fn install_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);

    let slashed = text.replace('\\', "/");
    let trimmed = slashed.strip_prefix("//?/").unwrap_or(&slashed);

    trimmed.trim_end_matches('/').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CN_ROOT: &str = "英雄联盟（含经典模式）";

    #[test]
    fn same_install_sets_aside_spelling_and_case() {
        let plain = PathBuf::from(format!(r"D:\WeGameApps\{CN_ROOT}"));

        let spellings = [
            format!(r"D:\WeGameApps\{CN_ROOT}"),
            format!(r"\\?\D:\WeGameApps\{CN_ROOT}"),
            format!("D:/WeGameApps/{CN_ROOT}"),
            format!("d:/wegameapps/{CN_ROOT}"),
            format!(r"D:\WeGameApps\{CN_ROOT}\"),
            format!("//?/D:/WeGameApps/{CN_ROOT}/"),
        ];

        for spelling in spellings {
            assert!(
                same_install(Path::new(&spelling), &plain),
                "{spelling} should name one install"
            );
        }
    }

    #[test]
    fn same_install_folds_ascii_case_without_touching_chinese() {
        let lower = PathBuf::from(format!(r"d:\wegameapps\{CN_ROOT}"));
        let upper = PathBuf::from(format!(r"D:\WeGameApps\{CN_ROOT}"));
        assert!(same_install(&lower, &upper));
    }

    #[test]
    fn same_install_tells_two_installs_apart() {
        assert!(!same_install(
            Path::new(r"D:\WeGameApps\英雄联盟（含经典模式）"),
            Path::new(r"D:\WeGameApps\League"),
        ));
    }
}
