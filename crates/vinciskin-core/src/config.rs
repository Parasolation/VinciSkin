//! Core application configuration.
//!
//! [`Config`] holds the settings shared by every frontend. UI preferences
//! (theme, tray behavior, hotkeys, …) live in the Tauri shell's `Settings`,
//! which embeds this struct via `#[serde(flatten)]` so everything persists to a
//! single `settings.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

fn default_keep_incidents() -> u32 {
    50
}

/// Core application configuration.
///
/// Every field is optional or defaulted so a partial (or empty) JSON document
/// deserializes into a usable configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[cfg_attr(feature = "ts", ts(as = "Option<String>"))]
    pub league_path: Option<PathBuf>,
    #[cfg_attr(feature = "ts", ts(as = "Option<String>"))]
    pub mod_storage_path: Option<PathBuf>,
    /// Directory where mod projects are stored (for Creator Workshop).
    #[cfg_attr(feature = "ts", ts(as = "Option<String>"))]
    pub workshop_path: Option<PathBuf>,
    /// Whether to automatically categorize mods from their content (champions,
    /// maps and content tags derived from the WAD/chunk footprint, surfaced as
    /// "auto" suggestions and library filters). When off, only the categories
    /// the user sets themselves are used. Default: true.
    #[serde(default = "default_true")]
    pub auto_categorization_enabled: bool,
    /// Enabling a mod moves it to the front of its folder. Off by default.
    #[serde(default)]
    pub promote_enabled_mods: bool,
    /// Whether to hide the Riot Client's window once the game is up. Nobody
    /// launching through the manager wants the launcher left sitting on their
    /// desktop behind the game, so this is on by default.
    ///
    /// Hides to the tray; the client keeps running because the game needs it for
    /// the whole session, and it stays hidden after the game exits. That last
    /// part takes active work: the client un-hides *itself* on exit, through the
    /// `showUxIfHidden` flag on Foundation's UX command bus, so
    /// `hide_for_play_session` re-asserts the hide. Reversible from the tray
    /// icon at any point. Default: true.
    #[serde(default = "default_true")]
    pub hide_riot_client_on_launch: bool,
    /// Whether to read League's own game log after a game ends, for the verdict
    /// on a game that went wrong. Turns the reader off. An incident still
    /// records the ending and the game's boundaries, and with this off the
    /// manager opens nothing under the League install. Default: true.
    #[serde(default = "default_true")]
    pub read_game_log: bool,
    /// How many incidents the app data directory keeps, under 1MB together.
    /// The oldest goes first, and a dismissed one before an undismissed one
    /// of the same age. Default: 50.
    #[serde(default = "default_keep_incidents")]
    pub keep_incidents: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            league_path: None,
            mod_storage_path: None,
            workshop_path: None,
            auto_categorization_enabled: true,
            promote_enabled_mods: false,
            hide_riot_client_on_launch: true,
            read_game_log: true,
            keep_incidents: default_keep_incidents(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let config = Config::default();
        assert!(config.league_path.is_none());
        assert!(config.mod_storage_path.is_none());
        assert!(config.workshop_path.is_none());
        assert!(config.auto_categorization_enabled);
        assert!(!config.promote_enabled_mods);
        assert!(config.hide_riot_client_on_launch);
        assert!(config.read_game_log);
        assert_eq!(config.keep_incidents, 50);
    }

    /// A config written before the patcher retired still carries its keys, so
    /// they have to parse as the noise they now are.
    #[test]
    fn a_config_with_retired_keys_still_parses() {
        let config: Config = serde_json::from_str(
            r#"{ "retainModArchives": false, "patchTft": true, "elevateInjector": true,
                 "wadBlocklist": ["Map12.wad.client"], "builtinMods": { "defaultWardSkins": true } }"#,
        )
        .unwrap();
        assert!(config.league_path.is_none());
    }

    #[test]
    fn config_deserializes_from_empty_object() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert!(config.league_path.is_none());
        assert!(config.hide_riot_client_on_launch);
        assert!(config.auto_categorization_enabled);
    }

    /// Settings files written before this flag existed have to come back on,
    /// not off - a missing key is an old install, not a user who said no.
    #[test]
    fn hiding_the_riot_client_defaults_on_for_settings_written_before_it_existed() {
        let config: Config = serde_json::from_str(r#"{ "patchTft": true }"#).unwrap();
        assert!(config.hide_riot_client_on_launch);
    }

    #[test]
    fn hiding_the_riot_client_can_be_turned_off() {
        let config: Config =
            serde_json::from_str(r#"{ "hideRiotClientOnLaunch": false }"#).unwrap();
        assert!(!config.hide_riot_client_on_launch);
    }

    /// The reader is on for an install whose settings predate it, and a user
    /// who turned it off stays off.
    #[test]
    fn reading_the_game_log_defaults_on_and_can_be_turned_off() {
        let config: Config = serde_json::from_str(r#"{ "patchTft": true }"#).unwrap();
        assert!(config.read_game_log);
        assert_eq!(config.keep_incidents, 50);

        let config: Config =
            serde_json::from_str(r#"{ "readGameLog": false, "keepIncidents": 10 }"#).unwrap();
        assert!(!config.read_game_log);
        assert_eq!(config.keep_incidents, 10);
    }
}
