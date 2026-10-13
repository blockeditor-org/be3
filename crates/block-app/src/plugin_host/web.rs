use std::{
    convert::Infallible,
    sync::{Arc, atomic::AtomicBool},
};

use block_plugin_api::{PluginManifest, ScreenDamage};

mod adapter;
mod canvases;
pub(super) mod presenter;

use super::backend::ShownFrame;
use adapter::WebProtocolAdapter;
pub(super) use adapter::WebProtocolAdapter as Channel;
pub(super) use canvases::{Placement, placement, set_background};

pub(super) struct Frame;

impl ShownFrame for Frame {
    fn presents(&self) -> u64 {
        0
    }

    fn damage(&self) -> Option<&[ScreenDamage]> {
        None
    }

    fn set_damage(&mut self, _damage: Option<Vec<ScreenDamage>>) {}
}

pub(super) fn spawn(
    _plugin: &PluginManifest,
    module: String,
    waiting: Arc<AtomicBool>,
) -> Result<Channel, String> {
    WebProtocolAdapter::start(&module, waiting)
}

pub(super) fn frame(_presented: Vec<Infallible>, _presents: u64) -> Frame {
    Frame
}
