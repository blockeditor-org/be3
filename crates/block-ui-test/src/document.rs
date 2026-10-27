use std::ops::{Deref, DerefMut};
use std::time::Duration;

use beui::native::Document;
use beui::native::WindowSize;
use beui::native::testing::{Showing, Stepped, Target};
use beui::{
    Color32, Rect, Vec2, native::Context, native::FontLibrary, native::FrameOutput,
    native::FreetypeFonts,
};

use crate::beui::capture;
use crate::overflow;
use crate::snapshot;

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

pub struct DocumentTest {
    stepped: Stepped,
    fonts: FontLibrary,
    latest: Option<FrameOutput>,
    recorded: Option<snapshot::Snapshot>,
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
        let mut test = Self {
            stepped,
            fonts,
            latest: None,
            recorded: None,
        };
        test.frame(Vec::new());
        test
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }

    pub fn frame(&mut self, inputs: Vec<beui::Input>) {
        self.latest = Some(self.stepped.frame(inputs));
    }

    pub fn shows(&self, test_id: &str) -> bool {
        let document = self.document();
        document
            .find_test_id(test_id)
            .is_some_and(|node| document.node_rect(node).is_some())
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        let document = self.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        document
            .node_rect(node)
            .unwrap_or_else(|| panic!("{test_id} is not laid out"))
    }

    pub fn click(&mut self, target: impl Into<Target>) -> String {
        let target = target.into();
        if let Target::Id(id, _) = &target {
            self.reveal(id);
        }
        let changed = self.stepped.click(target);
        self.stepped.leave();
        changed
    }

    pub fn scroll(&mut self, target: impl Into<Target>, by: Vec2) -> String {
        let changed = self.stepped.scroll(target, by);
        self.stepped.leave();
        changed
    }

    pub fn reveal(&mut self, test_id: &str) {
        let shown = Rect::from_min_size(beui::Pos2::ZERO, self.size());
        if self.rect_of(test_id).intersect(shown).is_positive() {
            return;
        }
        let document = self.document_mut();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        document.reveal_node(node);
        self.frame(Vec::new());
    }

    pub fn record(&mut self) {
        let size = self.size();
        self.refresh();
        let output = self
            .latest
            .as_ref()
            .expect("the document has not drawn a frame yet");
        let painting = capture::capture(
            output.main(),
            size,
            output.pixels_per_point(),
            background(output),
        )
        .expect("the painting could not be rendered");
        match &mut self.recorded {
            Some(recorded) => recorded.append(painting),
            None => self.recorded = Some(painting),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        self.record();
        let recorded = self.recorded.take().expect("a frame was just recorded");
        snapshot::assert_snapshot(name, &recorded);
        overflow::assert_fits(name, overflow::overflows(self.stepped.document()));
    }

    pub fn snapshot_window(&mut self, name: &str, window: beui::WindowId) {
        self.refresh();
        let output = self
            .stepped
            .windows()
            .output(window)
            .cloned()
            .unwrap_or_else(|| panic!("window {window:?} has painted nothing"));
        let background = self.latest.as_ref().map_or(Color32::BLACK, background);
        let painting = capture::capture_window(&output, 1.0, background)
            .expect("the painting could not be rendered");
        snapshot::assert_snapshot(name, &painting);
    }

    fn refresh(&mut self) {
        if self.stepped.output().is_some() {
            self.latest = Some(self.stepped.take_output());
        }
    }
}

fn background(output: &FrameOutput) -> Color32 {
    output.main().window.appearance.clear_color
}
