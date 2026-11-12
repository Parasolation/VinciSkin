//! Where a skin lives, for the static swap.
//!
//! Everything the manager used to build for a running game - the overlay, its artifacts and the
//! built-in mods above them - was retired with the patcher (ADR-0058). What is left here is the
//! three readers a swap needs: the catalog id to chunk-number mapping, the chunk path, and
//! whether an install holds that chunk at all, plus the bin that gives one skin another's
//! content (ADR-0056).

mod skins;

pub(crate) use skins::SkinContent;
pub use skins::{install_holds_skin, skin_bin_path, skin_number};
