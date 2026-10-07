use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenDamage, ScreenLayout};

mod adapter;
mod canvases;
pub(super) mod presenter;

use super::backend::{Backend, ShownFrame};
use adapter::WebProtocolAdapter;
use canvases::Canvases;
pub(super) use canvases::{Placement, placement, set_background};

pub(super) struct Web {
    url: String,
    adapter: Option<WebProtocolAdapter>,
    canvases: Canvases,
    started: f64,
    error: Option<String>,
}

impl Web {
    pub(super) fn place(&mut self, placements: Vec<Placement>) {
        let Some(adapter) = &self.adapter else {
            self.canvases.clear();
            return;
        };
        if let Err(error) = self.canvases.place(adapter, placements) {
            self.error.get_or_insert(error);
        }
    }
}

impl ShownFrame for () {
    fn presents(&self) -> u64 {
        0
    }

    fn damage(&self) -> Option<&[ScreenDamage]> {
        None
    }

    fn set_damage(&mut self, _damage: Option<Vec<ScreenDamage>>) {}
}

impl Backend for Web {
    type Frame = ();

    fn new(plugin: &PluginManifest) -> Self {
        Self {
            url: crate::editors::plugin::discovery::entry_point(
                &plugin.identity.id,
                &plugin.entry_point,
            )
            .unwrap_or_default(),
            adapter: None,
            canvases: Canvases::default(),
            started: now(),
            error: None,
        }
    }

    fn start(&mut self, _plugin: &PluginManifest) {
        self.shutdown();
        self.started = now();
        self.error = None;
        match WebProtocolAdapter::start(&self.url) {
            Ok(adapter) => self.adapter = Some(adapter),
            Err(error) => self.error = Some(error),
        }
    }

    fn ready(&self) -> bool {
        self.adapter.as_ref().is_some_and(WebProtocolAdapter::ready)
    }

    fn send(&mut self, messages: Vec<Message>) {
        let Some(adapter) = &mut self.adapter else {
            return;
        };
        if let Err(error) = adapter.send(messages) {
            self.error = Some(error);
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        let Some(adapter) = &mut self.adapter else {
            return Vec::new();
        };
        match adapter.poll() {
            Ok(messages) => messages,
            Err(error) => {
                self.error = Some(error);
                Vec::new()
            }
        }
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<()> {
        None
    }

    fn received_frame(&mut self) -> Option<()> {
        None
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match &self.adapter {
            Some(adapter) if adapter.running() => "running",
            Some(_) => "starting",
            None => "stopped",
        }
    }

    fn uptime(&self) -> Option<Duration> {
        self.adapter
            .is_some()
            .then(|| Duration::from_secs_f64((now() - self.started).max(0.0) / 1000.0))
    }

    fn shutdown(&mut self) {
        self.canvases.clear();
        if let Some(mut adapter) = self.adapter.take() {
            adapter.shutdown();
        }
    }
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}
