use std::ops::{Deref, DerefMut};
use std::time::Duration;

use beui::native::Document;
use beui::native::WindowSize;
use beui::native::testing::{Gui, Showing, Stepped};
use beui::{
    Color32, Vec2, native::Context, native::FontLibrary, native::FrameOutput, native::FreetypeFonts,
};

use crate::beui::capture;
use crate::overflow;
use crate::snapshot::{self, Recording, Snapshot, Snapshots};

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

pub struct DocumentTest {
    stepped: Stepped,
    fonts: FontLibrary,
    recording: Recording,
}

impl Deref for DocumentTest {
    type Target = Stepped;

    fn deref(&self) -> &Stepped {
        &self.stepped
    }
}

impl DerefMut for DocumentTest {
    fn deref_mut(&mut self) -> &mut Stepped {
        &mut self.stepped
    }
}

impl DocumentTest {
    pub fn new(document: Document, size: Vec2) -> Self {
        Self::start(document, size, false)
    }

    pub fn hosting_windows(document: Document, size: Vec2) -> Self {
        Self::start(document, size, true)
    }

    fn start(document: Document, size: Vec2, hosting: bool) -> Self {
        beui::native::fail_unsettled_layout(true);
        let fonts = FontLibrary::bundled();
        let context = Context::new(FreetypeFonts::new(fonts.clone()));
        let window = WindowSize { size, scale: 1.0 };
        let mut stepped = match hosting {
            true => Stepped::hosting_windows(context, Showing(document), window),
            false => Stepped::new(context, Showing(document), window, None),
        };
        stepped.settle_after_gestures(false);
        stepped.run();
        Self {
            stepped,
            fonts,
            recording: Recording::default(),
        }
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }

    pub fn snapshot_window(&mut self, name: &str, window: beui::WindowId) {
        let output = self
            .stepped
            .windows()
            .output(window)
            .cloned()
            .unwrap_or_else(|| panic!("window {window:?} has painted nothing"));
        let background = background(self.stepped.output());
        let painting = capture::capture_window(&output, 1.0, background)
            .expect("the painting could not be rendered");
        snapshot::assert_snapshot(name, &painting);
    }
}

impl Snapshots for DocumentTest {
    fn capture(&mut self) -> Snapshot {
        let size = self.stepped.size();
        let output = self.stepped.output();
        capture::capture(
            output.main(),
            size,
            output.pixels_per_point(),
            background(output),
        )
        .expect("the painting could not be rendered")
    }

    fn out_of_bounds(&self) -> Vec<String> {
        overflow::overflows(&self.stepped.document())
    }

    fn recording(&mut self) -> &mut Recording {
        &mut self.recording
    }
}

fn background(output: &FrameOutput) -> Color32 {
    output.main().window.appearance.clear_color
}
