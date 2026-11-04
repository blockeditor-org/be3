use beui_core::clipboard::{Clip, PasteAnswer};
use beui_core::window::{MAX_PIXELS, Resize, WindowReports, WindowSize};
use beui_runner::PlatformServices;
use beui_types::Vec2;

/// The headless platform's window: keeps the clip last copied, which pastes
/// back every representation it holds, grows to `screen` for fullscreen, and reports its size,
/// scale and fullscreen through `WindowReports` like a real window. It is the
/// only window `PlatformServices::resize` works on.
pub struct Simulation {
    pub window: WindowSize,
    pub screen: Vec2,
    pub fullscreen: bool,
    pub clipboard: Option<Clip>,
    reports: WindowReports,
}

impl Simulation {
    pub fn new(window: WindowSize, screen: Option<Vec2>, reports: WindowReports) -> Self {
        let simulation = Self {
            window,
            screen: screen.unwrap_or(window.size),
            fullscreen: false,
            clipboard: None,
            reports,
        };
        simulation.report_size();
        simulation
    }

    pub fn shown(&self) -> Vec2 {
        match self.fullscreen {
            true => self.screen,
            false => self.window.size,
        }
    }

    pub fn physical(&self) -> (u32, u32) {
        let shown = self.shown() * self.window.scale;
        (
            shown.x.round().max(1.0) as u32,
            shown.y.round().max(1.0) as u32,
        )
    }

    fn report_size(&self) {
        let (width, height) = self.physical();
        self.reports.resized(width, height);
        self.reports.scaled(self.window.scale);
    }
}

impl PlatformServices for Simulation {
    fn copy(&mut self, clip: Clip) {
        self.clipboard = Some(clip);
    }

    fn paste(&mut self, formats: &[String], answer: PasteAnswer) {
        match &self.clipboard {
            Some(clip) => clip.resolve(formats, move |item| answer.send(item)),
            None => answer.send(None),
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        if self.fullscreen != fullscreen {
            self.fullscreen = fullscreen;
            self.reports.fullscreen(fullscreen);
            self.report_size();
        }
    }

    fn resize(&mut self, resize: Resize) -> Result<(), String> {
        let (window, screen) = match resize {
            Resize::Window(window) => (window, self.screen),
            Resize::Screen(screen) => (self.window, screen),
        };
        let shown = WindowSize {
            size: screen,
            scale: window.scale,
        };
        if !shown.fits() {
            return Err(format!(
                "a {}x{} screen at scale {} is more than {} pixels across",
                screen.x, screen.y, window.scale, MAX_PIXELS
            ));
        }
        self.window = window;
        self.screen = screen;
        self.report_size();
        Ok(())
    }
}
