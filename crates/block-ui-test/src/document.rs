use std::ops::{Deref, DerefMut};

use beui::native::Document;
use beui::native::WindowSize;
use beui::{Rect, Vec2, native::Context};
use beui_host::native::testing::{Showing, Stepped, Target};
use beui_host::{native::FontLibrary, native::FreetypeFonts};

pub struct DocumentTest {
    stepped: Stepped,
    fonts: FontLibrary,
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
        stepped.frame(Vec::new());
        Self { stepped, fonts }
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
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
        let window = Rect::from_min_size(beui::Pos2::ZERO, self.size());
        if !scroll_cuts(&self.document(), test_id) && window.contains_rect(self.rect_of(test_id)) {
            return;
        }
        let document = self.document_mut();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        document.reveal_node(node);
        self.frame(Vec::new());
    }
}

pub(crate) fn scroll_cuts(document: &Document, test_id: &str) -> bool {
    document
        .find_test_ids(test_id)
        .into_iter()
        .any(|node| document.is_cut(node))
}
