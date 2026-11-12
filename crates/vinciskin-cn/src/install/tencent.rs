//! Installed patchlines from the Tencent (Chinese) client.

use std::path::{Path, PathBuf};

use super::{InstallSource, InstalledPatchline};

/// The Tencent install key. The real one lives under the current user.
pub const LOL_KEY: &str = r"SOFTWARE\Tencent\LOL";

/// Value names the client writes its root under, in probe order.
const INSTALL_VALUES: [&str; 2] = ["InstallPath", "InstallLocation"];

/// A registry hive a probe reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKEY_CURRENT_USER`, where the real key is written.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE`, the fallback some installs use.
    LocalMachine,
}

/// The hives probed, in order.
const HIVES: [Hive; 2] = [Hive::CurrentUser, Hive::LocalMachine];

/// One string value read out of the Windows registry.
pub trait RegistryProbe {
    /// The string at `path`'s `value`, `None` when it is absent or unreadable.
    fn read_string(&self, hive: Hive, path: &str, value: &str) -> Option<String>;
}

/// Tencent installs, discovered through the client's registry key.
pub struct TencentInstallSource<P: RegistryProbe> {
    probe: P,
}

impl<P: RegistryProbe> TencentInstallSource<P> {
    /// Wrap a registry probe.
    pub fn new(probe: P) -> Self {
        Self { probe }
    }

    fn find_root(&self) -> Option<PathBuf> {
        for hive in HIVES {
            for value in INSTALL_VALUES {
                let Some(raw) = self.probe.read_string(hive, LOL_KEY, value) else {
                    continue;
                };

                if raw.trim().is_empty() {
                    continue;
                }

                return Some(patchline_root(Path::new(&raw)));
            }
        }

        None
    }
}

impl<P: RegistryProbe> InstallSource for TencentInstallSource<P> {
    fn discover(&self) -> Vec<InstalledPatchline> {
        let Some(root) = self.find_root() else {
            return Vec::new();
        };

        vec![InstalledPatchline {
            id: "tencent".into(),
            root,
        }]
    }
}

/// The folder that holds `Game`, walking up when given the `Game` folder itself.
pub fn patchline_root(path: &Path) -> PathBuf {
    let is_game = path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Game"));

    if is_game {
        return path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
    }

    path.to_path_buf()
}

/// A League install under a WeGame root, found by scanning `WeGameApps/Games`.
pub fn find_in_wegame_root(root: &Path) -> Option<PathBuf> {
    let mut level = vec![root.join("WeGameApps").join("Games")];

    for _ in 0..2 {
        let mut next = Vec::new();

        for directory in level {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();

                if !path.is_dir() {
                    continue;
                }

                if path.join("Game").join("Data").is_dir() {
                    return Some(path);
                }

                next.push(path);
            }
        }

        level = next;
    }

    None
}

/// The Tencent key, read through `winreg` when the `winreg` feature is on.
#[cfg(windows)]
pub struct WinRegistryProbe;

#[cfg(all(windows, feature = "winreg"))]
impl RegistryProbe for WinRegistryProbe {
    fn read_string(&self, hive: Hive, path: &str, value: &str) -> Option<String> {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;

        let root = match hive {
            Hive::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
            Hive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
        };

        root.open_subkey_with_flags(path, KEY_READ)
            .ok()?
            .get_value::<String, _>(value)
            .ok()
    }
}

#[cfg(all(windows, not(feature = "winreg")))]
impl RegistryProbe for WinRegistryProbe {
    fn read_string(&self, _hive: Hive, _path: &str, _value: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CN_ROOT: &str = r"D:\WeGameApps\英雄联盟（含经典模式）";

    #[derive(Default)]
    struct FakeProbe {
        entries: Vec<(Hive, String, String, String)>,
    }

    impl FakeProbe {
        fn with(mut self, hive: Hive, path: &str, value: &str, result: &str) -> Self {
            self.entries.push((
                hive,
                path.to_string(),
                value.to_string(),
                result.to_string(),
            ));
            self
        }
    }

    impl RegistryProbe for FakeProbe {
        fn read_string(&self, hive: Hive, path: &str, value: &str) -> Option<String> {
            self.entries
                .iter()
                .find(|(h, p, v, _)| *h == hive && p == path && v == value)
                .map(|(_, _, _, result)| result.clone())
        }
    }

    #[test]
    fn the_current_user_key_supplies_the_install_root() {
        let probe = FakeProbe::default().with(Hive::CurrentUser, LOL_KEY, "InstallPath", CN_ROOT);

        let found = TencentInstallSource::new(probe).discover();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "tencent");
        assert_eq!(found[0].root, PathBuf::from(CN_ROOT));
    }

    #[test]
    fn the_local_machine_key_is_probed_when_the_current_user_key_is_absent() {
        let probe = FakeProbe::default().with(
            Hive::LocalMachine,
            LOL_KEY,
            "InstallLocation",
            r"D:\WeGameApps\League",
        );

        let found = TencentInstallSource::new(probe).discover();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].root, PathBuf::from(r"D:\WeGameApps\League"));
    }

    #[test]
    fn install_location_is_probed_when_install_path_is_absent() {
        let probe = FakeProbe::default()
            .with(Hive::CurrentUser, LOL_KEY, "InstallPath", "   ")
            .with(
                Hive::CurrentUser,
                LOL_KEY,
                "InstallLocation",
                r"D:\WeGameApps\League",
            );

        let found = TencentInstallSource::new(probe).discover();

        assert_eq!(found[0].root, PathBuf::from(r"D:\WeGameApps\League"));
    }

    #[test]
    fn a_root_that_ends_in_game_is_walked_up_to_its_parent() {
        let probe = FakeProbe::default().with(
            Hive::CurrentUser,
            LOL_KEY,
            "InstallPath",
            r"D:\WeGameApps\英雄联盟（含经典模式）\Game",
        );

        let found = TencentInstallSource::new(probe).discover();

        assert_eq!(found[0].root, PathBuf::from(CN_ROOT));
    }

    #[test]
    fn an_absent_key_yields_no_patchline() {
        let found = TencentInstallSource::new(FakeProbe::default()).discover();
        assert!(found.is_empty());
    }

    #[test]
    fn patchline_root_needs_the_folder_holding_game() {
        assert_eq!(
            patchline_root(Path::new(r"D:\x\Game")),
            PathBuf::from(r"D:\x")
        );
        assert_eq!(
            patchline_root(Path::new(r"D:\x\game")),
            PathBuf::from(r"D:\x")
        );
        assert_eq!(patchline_root(Path::new(r"D:\x")), PathBuf::from(r"D:\x"));
    }

    #[test]
    fn a_wegame_root_is_scanned_for_a_game_data_folder() {
        let root = std::env::temp_dir().join(format!("vinciskin-cn-wegame-{}", std::process::id()));
        let install = root.join("WeGameApps").join("Games").join("League");
        std::fs::create_dir_all(install.join("Game").join("Data")).unwrap();

        let found = find_in_wegame_root(&root);

        std::fs::remove_dir_all(&root).ok();
        assert_eq!(found, Some(install));
    }

    #[test]
    fn a_wegame_root_without_a_game_data_folder_yields_nothing() {
        let root = std::env::temp_dir().join(format!("vinciskin-cn-empty-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        let found = find_in_wegame_root(&root);

        std::fs::remove_dir_all(&root).ok();
        assert_eq!(found, None);
    }
}
