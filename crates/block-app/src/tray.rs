mod client;
mod icons;
mod model;
#[cfg(test)]
mod tests;
mod watcher;

use std::sync::mpsc::Receiver;

use block_plugin_api::HostTrayItem;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use client::Event;

pub(crate) struct Tray {
    events: UnboundedSender<Event>,
    items: Receiver<Vec<HostTrayItem>>,
}

impl Tray {
    pub(crate) fn start() -> Self {
        let (published, items) = crate::host::waking_channel();
        let (events, heard) = unbounded_channel();
        crate::dbus::spawn(client::run(events.clone(), heard, move |listed| {
            let _ = published.send(listed);
        }));
        Self { events, items }
    }

    pub(crate) fn frame(&mut self) {
        for request in crate::plugin_host::take_calls(block_plugin_api::requested::tray) {
            let _ = self.events.send(Event::Request(request));
        }
        if let Some(listed) = self.items.try_iter().last() {
            crate::plugin_host::publish::<block_plugin_api::TrayWatch>(&listed);
        }
    }
}
