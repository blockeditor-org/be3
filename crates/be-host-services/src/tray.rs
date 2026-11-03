mod client;
mod icons;
mod model;
#[cfg(test)]
mod tests;
mod watcher;

use be_host_base::host::{self, Mailbox};
use be_sdk_api::{HostTrayItem, TrayWatch};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use client::Event;

pub struct Tray {
    events: UnboundedSender<Event>,
    _items: Mailbox,
}

impl Tray {
    pub fn start() -> Self {
        let (items, post) = host::mailbox(|listed: Vec<HostTrayItem>| {
            be_host_base::watches::publish::<TrayWatch>(&listed);
        });
        let (events, heard) = unbounded_channel();
        let sender = events.clone();
        crate::dbus::spawn(async move {
            client::serve(&crate::dbus::SESSION, sender, heard, move |listed| {
                post.send(listed);
            })
            .await;
        });
        Self {
            events,
            _items: items,
        }
    }

    pub fn frame(&mut self) {
        for request in be_host_base::watches::take_calls(be_sdk_api::requested::tray) {
            let _ = self.events.send(Event::Request(request));
        }
    }
}
