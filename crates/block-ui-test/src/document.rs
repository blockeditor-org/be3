use std::time::Duration;

use beui::{
    Color32, Context, Document, FontLibrary, FrameOutput, FreetypeFonts, Modifiers, PointerButton,
    Pos2, RawInput, Rect, Vec2,
};

use crate::beui::capture;
use crate::snapshot;

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const DRAG_STEPS: usize = 8;

pub struct DocumentTest {
    document: Document,
    context: Context,
    fonts: FontLibrary,
    size: Vec2,
    output: Option<FrameOutput>,
}

impl DocumentTest {
    pub fn new(document: Document, size: Vec2) -> Self {
        let fonts = FontLibrary::bundled();
        let context = Context::new(FreetypeFonts::new(fonts.clone()));
        context.stop_clock();
        let mut test = Self {
            document,
            context,
            fonts,
            size,
            output: None,
        };
        test.frame(Vec::new());
        test
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }

    pub fn frame(&mut self, events: Vec<beui::Event>) {
        let Self {
            document,
            context,
            size,
            ..
        } = self;
        context.advance_clock(FRAME_INTERVAL);
        let output = context.run(RawInput { events }, |context| {
            document.show(context, Rect::from_min_size(Pos2::ZERO, *size));
        });
        self.output = Some(output);
    }

    pub fn shows(&self, test_id: &str) -> bool {
        self.document
            .find_test_id(test_id)
            .is_some_and(|node| self.document.node_rect(node).is_some())
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        let node = self
            .document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        self.document
            .node_rect(node)
            .unwrap_or_else(|| panic!("{test_id} is not laid out"))
    }

    pub fn click(&mut self, test_id: &str) {
        let rect = self.rect_of(test_id);
        self.click_at(rect.center());
    }

    pub fn click_at(&mut self, pos: Pos2) {
        self.frame(vec![beui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            self.frame(vec![beui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.frame(vec![beui::Event::PointerGone]);
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![beui::Event::PointerMoved(from)]);
        self.frame(vec![beui::Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        for step in 1..=DRAG_STEPS {
            let along = step as f32 / DRAG_STEPS as f32;
            self.frame(vec![beui::Event::PointerMoved(from + (to - from) * along)]);
        }
        self.frame(vec![beui::Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub fn scroll_at(&mut self, pos: Pos2, delta: Vec2) {
        self.frame(vec![
            beui::Event::PointerMoved(pos),
            beui::Event::Scroll(delta),
        ]);
        self.frame(vec![beui::Event::ScrollEnded, beui::Event::PointerGone]);
    }

    pub fn snapshot(&mut self, name: &str) {
        let output = self
            .output
            .as_ref()
            .expect("the document has not drawn a frame yet");
        let painting =
            capture::capture(output, self.size, output.pixels_per_point(), Color32::BLACK)
                .expect("the painting could not be rendered");
        snapshot::assert_snapshot(name, &painting);
    }
}
