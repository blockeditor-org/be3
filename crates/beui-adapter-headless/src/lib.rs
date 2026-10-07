use std::error::Error;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};

use beui_core::app::{SafeArea, Setup, Waker};
use beui_core::file_picker::FilePickRequest;
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Launch, Platform, Runner, Running};

type Load = Box<dyn FnOnce(u32, u32) -> Result<Vec<Loaded>, Box<dyn Error>>>;

pub struct Headless {
    load: Load,
}

impl Headless {
    pub fn new(
        load: impl FnOnce(u32, u32) -> Result<Vec<Loaded>, Box<dyn Error>> + 'static,
    ) -> Self {
        Self {
            load: Box::new(load),
        }
    }
}

impl Adapter for Headless {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(launch, self.load) })
    }
}

#[derive(Default)]
struct Clipboard(Option<String>);

impl Platform for Clipboard {
    fn copy(&mut self, text: String) {
        self.0 = Some(text);
    }

    fn paste(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn pick_file(&mut self, _request: FilePickRequest) {}
}

fn run(launch: Launch, load: Load) -> Result<(), Box<dyn Error>> {
    let size = launch.options.size;
    let loaded = load(size.x as u32, size.y as u32)?;
    let (wake, woken) = channel();
    let mut runner = Runner::new(launch);
    runner.start(
        loaded,
        Setup::new(Waker::new(move || {
            let _ = wake.send(());
        })),
    )?;
    let mut platform = Clipboard::default();
    loop {
        let Some(frame) = runner.frame(&mut platform, 1.0, SafeArea::default()) else {
            return Err("the headless renderer has no size".into());
        };
        if frame.close_requested {
            break;
        }
        if frame.pending {
            runner.present();
        }
        if !frame.again() {
            wait(&woken, frame.repaint_after);
        }
    }
    runner.exit();
    Ok(())
}

fn wait(woken: &Receiver<()>, timeout: std::time::Duration) {
    match woken.recv_timeout(timeout) {
        Ok(()) | Err(RecvTimeoutError::Timeout) => while woken.try_recv().is_ok() {},
        Err(RecvTimeoutError::Disconnected) => {}
    }
}

#[cfg(test)]
mod tests;
