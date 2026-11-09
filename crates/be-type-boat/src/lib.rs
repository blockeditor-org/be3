mod boat;
mod settings;

pub use boat::{Boat, BoatContent};
pub use settings::{BoatKey, BoatSettings, BoatSettingsContent};

be_sdk_type_guest::be_block_types![BoatContent { undo }, BoatSettingsContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
