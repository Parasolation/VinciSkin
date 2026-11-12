//! What a mod actually touches, and what that makes it.
//!
//! [`scan`] drives the overlay builder's WAD analysis for a single mod;
//! [`categorize`] turns the result into champions, maps and tags.
//! [`wad_reports`] persists that footprint, because it is a stable property of
//! the mod.

pub(super) mod categorize;
pub(super) mod scan;
pub(super) mod wad_reports;
