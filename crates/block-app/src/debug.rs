mod client;
pub(crate) mod inspect;
mod network;
pub(crate) mod plugins;
pub(crate) mod version;

#[cfg(feature = "terminal")]
pub(crate) mod terminal;
