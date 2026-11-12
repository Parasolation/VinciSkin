//! Alignment of the two Tencent tables against Riot's Data Dragon version.

use crate::version::PatchVersion;

/// The three versions an alignment reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentInput {
    /// Riot's Data Dragon version, the only authority.
    pub riot: PatchVersion,
    /// The version the Chinese hero table reports.
    pub tencent_heroes: PatchVersion,
    /// The version the Chinese skin table reports.
    pub tencent_skins: PatchVersion,
}

/// What an alignment concludes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Both tables match Riot's major.minor and no revision stands out.
    Aligned,
    /// Heroes are ahead of Riot and skins are level. Keep the avatar cache.
    HeroesAheadKeepAvatarCache,
    /// Riot is ahead of the heroes table and skins are level. Keep the skin cache.
    RiotAheadKeepSkinCache,
    /// Skins lag an otherwise-matching hero table, so the patch is not complete.
    SkinsBehindNotComplete,
    /// A table's major.minor does not match Riot's, so the catalog is rejected.
    OutOfSync {
        /// `hero` or `skin`.
        table: &'static str,
    },
}

/// How many failed portraits stop the versions being re-queried.
pub const PORTRAIT_FAILURE_LIMIT: u32 = 2;

/// Both tables' major.minor must match Riot. Within a matching patchline a
/// revision gap picks which cache survives, and a skin table behind the heroes
/// means the patch is not complete.
pub fn align(input: &AlignmentInput) -> Verdict {
    let riot = (input.riot.major, input.riot.minor);

    if (input.tencent_heroes.major, input.tencent_heroes.minor) != riot {
        return Verdict::OutOfSync { table: "hero" };
    }

    if (input.tencent_skins.major, input.tencent_skins.minor) != riot {
        return Verdict::OutOfSync { table: "skin" };
    }

    if input.tencent_skins.revision < input.tencent_heroes.revision {
        return Verdict::SkinsBehindNotComplete;
    }

    if input.tencent_heroes.revision > input.riot.revision {
        return Verdict::HeroesAheadKeepAvatarCache;
    }

    if input.riot.revision > input.tencent_heroes.revision {
        return Verdict::RiotAheadKeepSkinCache;
    }

    Verdict::Aligned
}

/// The session state the client keeps around a catalog version.
#[derive(Debug, Clone, Default)]
pub struct CatalogCache {
    verdict: Option<Verdict>,
    portrait_failures: u32,
    failed_directory: bool,
}

impl CatalogCache {
    /// An empty cache, which nothing may be served from yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the cache may be served without re-querying.
    pub fn is_usable(&self) -> bool {
        match self.verdict {
            Some(Verdict::OutOfSync { .. }) | None => false,
            Some(_) => !self.failed_directory,
        }
    }

    /// Record a successful alignment, which clears any prior failure.
    pub fn on_aligned(&mut self, verdict: Verdict) {
        self.verdict = Some(verdict);
        self.portrait_failures = 0;
        self.failed_directory = false;
    }

    /// Record a failed portrait fetch, returning whether the versions should be
    /// queried again. The limit stops a broken directory being retried.
    pub fn on_portrait_failure(&mut self) -> bool {
        self.portrait_failures += 1;

        if self.portrait_failures >= PORTRAIT_FAILURE_LIMIT {
            self.failed_directory = true;
            return false;
        }

        true
    }

    /// The only path that clears a failed state, so the caller re-queries both versions.
    pub fn manual_refresh(&mut self) {
        self.verdict = None;
        self.portrait_failures = 0;
        self.failed_directory = false;
    }

    /// Whether a login preload may run. A failed directory is not retried here.
    pub fn preload_attempt(&mut self) -> bool {
        !self.failed_directory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERO_LIST_FIXTURE: &str = include_str!("../../tests/fixtures/hero_list.js");
    const HERO_SKINS_FIXTURE: &str = include_str!("../../tests/fixtures/hero_1.js");

    fn version(text: &str) -> PatchVersion {
        PatchVersion::parse(text).expect("test version parses")
    }

    #[test]
    fn the_two_real_tencent_catalogs_report_one_version() {
        let heroes = crate::catalog::gtimg::parse_hero_list(HERO_LIST_FIXTURE).unwrap();
        let skins = crate::catalog::gtimg::parse_hero_skins(HERO_SKINS_FIXTURE).unwrap();

        assert_eq!(skins.version, heroes.version);
        assert_eq!(heroes.version, version("16.19"));
    }

    fn input(riot: &str, heroes: &str, skins: &str) -> AlignmentInput {
        AlignmentInput {
            riot: version(riot),
            tencent_heroes: version(heroes),
            tencent_skins: version(skins),
        }
    }

    #[test]
    fn riot_and_tencent_versions_compare_by_major_and_minor_patch() {
        let riot = version("16.19.1");

        assert!(riot.same_patch(&version("16.19.7")));
        assert!(version("16.19").same_patch(&riot));
        assert!(!riot.same_patch(&version("16.20.1")));
    }

    #[test]
    fn a_tencent_hero_version_ahead_of_riot_leaves_the_avatar_cache_untouched() {
        let verdict = align(&input("16.19.1", "16.19.2", "16.19.2"));
        assert_eq!(verdict, Verdict::HeroesAheadKeepAvatarCache);
    }

    #[test]
    fn riot_ahead_of_tencent_heroes_leaves_the_skin_cache_untouched() {
        let verdict = align(&input("16.19.3", "16.19.1", "16.19.1"));
        assert_eq!(verdict, Verdict::RiotAheadKeepSkinCache);
    }

    #[test]
    fn tencent_skins_behind_matching_heroes_do_not_mark_the_patch_complete() {
        let verdict = align(&input("16.19.1", "16.19.1", "16.19.0"));
        assert_eq!(verdict, Verdict::SkinsBehindNotComplete);
    }

    #[test]
    fn both_tencent_tables_can_differ_in_revision_but_must_match_riots_patch() {
        let aligned = input("16.19.1", "16.19.1", "16.19.7");

        assert_ne!(aligned.tencent_heroes, aligned.tencent_skins);
        assert_eq!(align(&aligned), Verdict::Aligned);
    }

    #[test]
    fn tencent_hero_catalog_does_not_match_data_dragon_patch() {
        let verdict = align(&input("16.19.1", "16.20.1", "16.19.1"));
        assert_eq!(verdict, Verdict::OutOfSync { table: "hero" });
    }

    #[test]
    fn tencent_catalogs_are_out_of_sync() {
        let verdict = align(&input("16.19.1", "16.19.1", "16.20.0"));
        assert_eq!(verdict, Verdict::OutOfSync { table: "skin" });
    }

    #[test]
    fn a_hero_table_without_a_revision_still_shares_riots_patch() {
        let verdict = align(&input("16.19.1", "16.19", "16.19"));
        assert_ne!(verdict, Verdict::OutOfSync { table: "hero" });
    }

    #[test]
    fn a_hero_table_on_another_patch_is_rejected() {
        let verdict = align(&input("16.20.1", "16.19", "16.20"));
        assert_eq!(verdict, Verdict::OutOfSync { table: "hero" });
    }

    #[test]
    fn two_failed_tencent_portraits_do_not_requery_riot_versions() {
        let mut cache = CatalogCache::new();
        cache.on_aligned(Verdict::Aligned);

        assert!(cache.on_portrait_failure());
        assert!(!cache.on_portrait_failure());
        assert!(!cache.is_usable());
    }

    #[test]
    fn manual_avatar_refresh_rechecks_riot_and_tencent() {
        let mut cache = CatalogCache::new();
        cache.on_aligned(Verdict::Aligned);
        cache.on_portrait_failure();
        cache.on_portrait_failure();

        assert!(!cache.is_usable());

        cache.manual_refresh();

        assert!(!cache.is_usable());
        assert!(cache.preload_attempt());
    }

    #[test]
    fn login_preload_does_not_automatically_retry_a_failed_tencent_directory() {
        let mut cache = CatalogCache::new();
        cache.on_aligned(Verdict::Aligned);
        cache.on_portrait_failure();
        cache.on_portrait_failure();

        assert!(!cache.preload_attempt());
    }

    #[test]
    fn skin_service_checks_riot_and_reuses_a_fresh_matching_tencent_directory() {
        let mut cache = CatalogCache::new();
        assert!(!cache.is_usable());

        cache.on_aligned(align(&input("16.19.1", "16.19", "16.19")));

        assert!(cache.is_usable());
        assert!(cache.preload_attempt());
    }

    #[test]
    fn an_out_of_sync_catalog_is_never_usable() {
        let mut cache = CatalogCache::new();
        cache.on_aligned(Verdict::OutOfSync { table: "hero" });
        assert!(!cache.is_usable());
    }
}
