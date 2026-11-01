mod client;
mod icons;
mod model;
#[cfg(test)]
mod tests;
mod watcher;

use block_app_base::host::{self, Mailbox};
use block_plugin_api::{HostTrayItem, TrayWatch};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use client::Event;

pub struct Tray {
    events: UnboundedSender<Event>,
    _items: Mailbox,
}

impl Tray {
    pub fn start() -> Self {
        let (items, post) = host::mailbox(|listed: Vec<HostTrayItem>| {
            block_app_base::watches::publish::<TrayWatch>(&listed);
        });
        let (events, heard) = unbounded_channel();
        crate::dbus::spawn(client::run(events.clone(), heard, move |listed| {
            post.send(listed);
        }));
        Self {
            events,
            _items: items,
        }
    }

    pub fn frame(&mut self) {
        for request in block_app_base::watches::take_calls(block_plugin_api::requested::tray) {
            let _ = self.events.send(Event::Request(request));
        }
    }
}
