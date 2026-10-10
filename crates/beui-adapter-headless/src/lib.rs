#![cfg(not(target_arch = "wasm32"))]

use std::error::Error;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use beui_core::app::automation::{Simulation, WindowSize};
use beui_core::app::{SafeArea, Setup, Waker};
use beui_core::file_picker::FilePickRequest;
use beui_core::geometry::Vec2;
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Launch, Platform, Runner, Running};
use beui_renderer_wgpu::offscreen::OffscreenSurface;
use beui_renderer_wgpu::present::OpenDevice;

const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

pub struct Headless {
    pub window: WindowSize,
    pub screen: Option<Vec2>,
    pub open_device: Option<OpenDevice>,
}

impl Adapter for Headless {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(*self, launch) })
    }
}

struct Offscreen;

impl Platform for Offscreen {
    fn copy(&mut self, _text: String) {}

    fn paste(&mut self) -> Option<String> {
        None
    }

    fn pick_file(&mut self, _request: FilePickRequest) {}
}

fn run(headless: Headless, launch: Launch) -> Result<(), Box<dyn Error>> {
    let simulation = Simulation::new(headless.window, headless.screen);
    let (width, height) = simulation.physical();
    let renderer = pollster::block_on(OffscreenSurface::new(width, height, headless.open_device))?;
    let mut runner = Runner::new(launch);
    runner.simulate(simulation);
    let (sender, receiver) = mpsc::channel();
    let setup = Setup::new(Waker::new(move || {
        let _ = sender.send(());
    }));
    runner.start(
        vec![Loaded {
            renderer: Box::new(renderer),
            fonts: None,
        }],
        setup,
    )?;
    loop {
        let started = Instant::now();
        let next = match runner.frame(&mut Offscreen, headless.window.scale, SafeArea::default()) {
            Some(frame) => {
                if frame.close_requested {
                    break;
                }
                let again = runner.present();
                if frame.again() || again {
                    Some(started + FRAME_INTERVAL)
                } else {
                    started.checked_add(frame.repaint_after)
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
    runner.exit();
    Ok(())
}
