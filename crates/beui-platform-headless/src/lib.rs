#![cfg(not(target_arch = "wasm32"))]

use std::error::Error;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use beui_core::app::automation::{Simulation, WindowSize};
use beui_core::app::{SafeArea, Setup, Waker};
use beui_core::geometry::Vec2;
use beui_core::renderer::{Loaded, NoRenderer, Renderer};
use beui_core::runner::{Platform, Launch, Runner, Running};
use beui_renderer_wgpu::offscreen::OffscreenSurface;
use beui_renderer_wgpu::present::OpenDevice;

const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

pub struct Headless {
    pub window: WindowSize,
    pub screen: Option<Vec2>,
    pub open_device: Option<OpenDevice>,
    pub render: bool,
}

impl Platform for Headless {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(*self, launch) })
    }
}

fn run(headless: Headless, launch: Launch) -> Result<(), Box<dyn Error>> {
    let mut runner = Runner::new(launch);
    runner.set_occluded(true);
    let mut window = Simulation::new(headless.window, headless.screen, runner.reports().clone());
    let (width, height) = window.physical();
    let renderer: Box<dyn Renderer> = match headless.render {
        true => Box::new(pollster::block_on(OffscreenSurface::new(
            width,
            height,
            headless.open_device,
        ))?),
        false => Box::new(NoRenderer::new(width, height)),
    };
    let (sender, receiver) = mpsc::channel();
    let setup = Setup::new(Waker::new(move || {
        let _ = sender.send(());
    }));
    runner.start(
        vec![Loaded {
            renderer,
            fonts: None,
        }],
        setup,
    )?;
    loop {
        let started = Instant::now();
        let next = match runner.frame(&mut window, SafeArea::default()) {
            Some(frame) => {
                let presenting = runner.present();
                match frame.next(presenting) {
                    Some(Duration::ZERO) => Some(started + FRAME_INTERVAL),
                    Some(wait) => started.checked_add(wait),
                    None => None,
                }
            }
            None if runner.exited() => break,
            None => None,
        };
        let woken = match next {
            Some(at) => match receiver.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => true,
                Err(RecvTimeoutError::Disconnected) => false,
            },
            None => receiver.recv().is_ok(),
        };
        if !woken {
            break;
        }
        while receiver.try_recv().is_ok() {}
    }
    runner.finish()
}
