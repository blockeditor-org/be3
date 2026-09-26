use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};

mod adapter;
pub(super) mod renderer;

use super::backend::Backend;
use adapter::WebProtocolAdapter;

pub(super) struct Web {
    url: String,
    adapter: Option<WebProtocolAdapter>,
    started: f64,
    error: Option<String>,
}

impl Backend for Web {
    type Frame = renderer::WebFrame;

    fn new(plugin: &PluginManifest) -> Self {
        Self {
            url: crate::editors::plugin::discovery::entry_point(
                &plugin.identity.id,
                &plugin.entry_point,
            )
            .unwrap_or_default(),
            adapter: None,
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
        if let Err(error) = adapter.poll() {
            self.error = Some(error);
            return Vec::new();
        }
        adapter.take_received()
    }

    fn frame(&mut self, layout: &ScreenLayout, _pass: u64) -> Option<Self::Frame> {
        Some(renderer::WebFrame {
            size: [layout.width, layout.height],
            picture: self.adapter.as_ref().and_then(WebProtocolAdapter::picture),
        })
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
