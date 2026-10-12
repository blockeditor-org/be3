mod client;
mod icons;
mod model;
#[cfg(test)]
mod tests;
mod watcher;

use std::sync::mpsc::Receiver;

use block_plugin_api::{HostTrayItem, TrayRequest};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use client::Event;

pub(crate) struct Tray {
    events: UnboundedSender<Event>,
    items: Receiver<Vec<HostTrayItem>>,
}

impl Tray {
    pub(crate) fn start() -> Self {
        let (published, items) = block_app_lib::extension::waking_channel();
        let (events, heard) = unbounded_channel();
        crate::dbus::spawn(client::run(events.clone(), heard, move |listed| {
            let _ = published.send(listed);
        }));
        Self { events, items }
    }

    pub(crate) fn frame(&mut self) {
        for request in block_app_lib::extension::take_actions::<TrayRequest>() {
            let _ = self.events.send(Event::Request(request));
        }
        if let Some(listed) = self.items.try_iter().last() {
            block_app_lib::extension::publish::<block_plugin_api::Tray>(&listed);
        }
    }
}
