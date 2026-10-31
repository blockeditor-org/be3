use stable_hash::HashSet;
use std::sync::{Arc, atomic::AtomicBool};

use block_plugin_api::ScreenDamage;
use block_plugin_manifest::PluginManifest;

mod canvases;
mod overlay;
pub(super) mod presenter;
mod worker;

use super::backend::ShownFrame;
pub(super) use canvases::{Placement, placement, set_background};
pub(super) use overlay::{Overlay, overlays};
use worker::WorkerChannel;
pub(super) use worker::{PresentedBitmap, WorkerChannel as Channel};

pub(super) type Picture = web_sys::ImageBitmap;

pub(super) type Mailbox = ();

pub(super) struct Frame {
    pictures: Vec<PresentedBitmap>,
}

impl ShownFrame for Frame {
    fn presents(&self) -> u64 {
        0
    }

    fn damage(&self) -> Option<&[ScreenDamage]> {
        None
    }

    fn set_damage(&mut self, _damage: Option<Vec<ScreenDamage>>) {}

    fn composited(&self) -> bool {
        false
    }
}

pub(super) fn prepare(_plugin: &PluginManifest, _module: String) {}

pub(super) fn hand_over_device() {}

pub(super) fn discard_prepared() {}

pub(super) fn spawn(
    _plugin: &PluginManifest,
    module: String,
) -> Result<(Channel, Mailbox, Arc<AtomicBool>), String> {
    let waiting = Arc::<AtomicBool>::default();
    WorkerChannel::start(&module, Arc::clone(&waiting)).map(|channel| (channel, (), waiting))
}

pub(super) fn frame(presented: Vec<PresentedBitmap>, _presents: u64) -> Frame {
    Frame {
        pictures: presented,
    }
}

pub(super) fn offer(_mailbox: &Mailbox, channel: &mut Channel, image: u64, picture: &Picture) {
    channel.offer(image, picture);
}

pub(super) fn withdraw(_mailbox: &Mailbox, channel: &mut Channel, image: u64) {
    channel.withdraw(image);
}

pub(super) fn export(channel: &mut Channel, surfaces: Vec<u32>) {
    channel.export(surfaces.into_iter().collect::<HashSet<u32>>());
}

pub(super) fn release(picture: &Picture) {
    picture.close();
}

pub(super) fn pictures(frame: &Frame, surfaces: &[u32]) -> Vec<(u32, Picture)> {
    frame
        .pictures
        .iter()
        .filter(|presented| surfaces.contains(&presented.surface))
        .map(|presented| (presented.surface, presented.bitmap.clone()))
        .collect()
}
