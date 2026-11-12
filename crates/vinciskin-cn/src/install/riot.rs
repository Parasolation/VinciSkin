//! Installed patchlines from the Riot Client's product registry.

use super::{InstallSource, InstalledPatchline};

/// The Riot product registry. A real implementation delegates to `ritoclient`.
pub trait ProductRegistry {
    /// Every League patchline the registry lists.
    fn installed_patchlines(&self) -> Vec<InstalledPatchline>;
}

/// Riot installs, discovered through the Riot Client.
pub struct RiotInstallSource<R: ProductRegistry> {
    registry: R,
}

impl<R: ProductRegistry> RiotInstallSource<R> {
    /// Wrap a product registry.
    pub fn new(registry: R) -> Self {
        Self { registry }
    }
}

impl<R: ProductRegistry> InstallSource for RiotInstallSource<R> {
    fn discover(&self) -> Vec<InstalledPatchline> {
        self.registry.installed_patchlines()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Default)]
    struct FakeRegistry {
        patchlines: Vec<InstalledPatchline>,
    }

    impl ProductRegistry for FakeRegistry {
        fn installed_patchlines(&self) -> Vec<InstalledPatchline> {
            self.patchlines.clone()
        }
    }

    #[test]
    fn riot_discovery_delegates_to_the_product_registry() {
        let registry = FakeRegistry {
            patchlines: vec![InstalledPatchline {
                id: "live".into(),
                root: PathBuf::from(r"C:\Riot Games\League of Legends"),
            }],
        };

        let found = RiotInstallSource::new(registry).discover();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "live");
        assert_eq!(
            found[0].root,
            PathBuf::from(r"C:\Riot Games\League of Legends")
        );
    }

    #[test]
    fn a_silent_product_registry_yields_nothing() {
        assert!(RiotInstallSource::new(FakeRegistry::default())
            .discover()
            .is_empty());
    }
}
