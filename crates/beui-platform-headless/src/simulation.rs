use beui_core::clipboard::PasteAnswer;
use beui_core::window::{MAX_PIXELS, Resize, WindowReports, WindowSize};
use beui_runner::PlatformServices;
use beui_types::PickedFile;
use beui_types::Vec2;
use beui_types::{PasteKind, Pasted};

pub struct Simulation {
    pub window: WindowSize,
    pub screen: Vec2,
    pub fullscreen: bool,
    pub clipboard: Option<String>,
    pub copied_image: Option<Vec<u8>>,
    reports: WindowReports,
}

impl Simulation {
    pub fn new(window: WindowSize, screen: Option<Vec2>, reports: WindowReports) -> Self {
        let simulation = Self {
            window,
            screen: screen.unwrap_or(window.size),
            fullscreen: false,
            clipboard: None,
            copied_image: None,
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
    fn copy(&mut self, text: String) {
        self.copied_image = None;
        self.clipboard = Some(text);
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        self.clipboard = None;
        self.copied_image = Some(png);
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let text = self
            .clipboard
            .clone()
            .filter(|_| kinds.contains(&PasteKind::Text))
            .map(Pasted::Text);
        let image = || {
            self.copied_image
                .clone()
                .filter(|_| kinds.contains(&PasteKind::Image))
                .map(|data| {
                    Pasted::Image(PickedFile {
                        name: "Pasted Image.png".to_owned(),
                        data,
                    })
                })
        };
        answer.send(text.or_else(image));
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
