use std::time::Duration;

use block_gpu_host::{Call, Gpu};
use block_plugin_api::{Message, PluginManifest, ScreenLayout};

mod adapter;

use super::backend::Backend;
use super::surface::{SurfaceFrame, gpu};
use adapter::WebProtocolAdapter;

const SCREENS_SURFACE: u32 = 0;
const NO_GPU: &str = "The plugin host has no graphics device.";

pub(super) struct Web {
    url: String,
    adapter: Option<WebProtocolAdapter>,
    gpu: Option<Gpu>,
    presented: bool,
    started: f64,
    error: Option<String>,
}

impl Backend for Web {
    type Frame = SurfaceFrame;

    fn new(plugin: &PluginManifest) -> Self {
        Self {
            url: crate::editors::plugin::discovery::entry_point(
                &plugin.identity.id,
                &plugin.entry_point,
            )
            .unwrap_or_default(),
            adapter: None,
            gpu: None,
            presented: false,
            started: now(),
            error: None,
        }
    }

    fn start(&mut self, _plugin: &PluginManifest) {
        self.shutdown();
        self.started = now();
        self.error = None;
        let Some((device, queue)) = gpu() else {
            self.error = Some(NO_GPU.to_owned());
            return;
        };
        let gpu = Gpu::new(device, queue);
        match WebProtocolAdapter::start(&self.url, &gpu.limits()) {
            Ok(adapter) => {
                self.adapter = Some(adapter);
                self.gpu = Some(gpu);
            }
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
        let (Some(adapter), Some(gpu)) = (&mut self.adapter, &mut self.gpu) else {
            return Vec::new();
        };
        let deliveries = match adapter.poll() {
            Ok(deliveries) => deliveries,
            Err(error) => {
                self.error = Some(error);
                return Vec::new();
            }
        };
        let mut received = Vec::new();
        for delivery in deliveries {
            match block_gpu_abi::decode::<Vec<Call>>(&delivery.calls) {
                Ok(calls) => calls.into_iter().for_each(|call| gpu.apply(call)),
                Err(error) => {
                    self.error.get_or_insert(error);
                }
            }
            received.extend(delivery.messages);
        }
        if gpu.take_presented().contains(&SCREENS_SURFACE) {
            self.presented = true;
        }
        if let Some(error) = gpu.take_error() {
            self.error.get_or_insert(error);
        }
        received
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<SurfaceFrame> {
        None
    }

    fn received_frame(&mut self) -> Option<SurfaceFrame> {
        if !std::mem::take(&mut self.presented) {
            return None;
        }
        let (texture, generation) = self.gpu.as_ref()?.surface(SCREENS_SURFACE)?;
        Some(SurfaceFrame {
            texture: texture.clone(),
            generation,
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
        self.gpu = None;
        self.presented = false;
    }
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}
