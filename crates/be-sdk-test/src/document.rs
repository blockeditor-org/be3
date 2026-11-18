use std::ops::{Deref, DerefMut};

use beui::Vec2;
use beui::native::Context;
use beui::native::Document;
use beui::native::WindowSize;
use beui_host::native::testing::{Gui, Showing, Stepped, Wraps};
use beui_host::{native::FontLibrary, native::FreetypeFonts};

/// Paints a beui document that is not an editor (beui-demo's pages), with only beui's
/// bundled fonts. It is beui's `testing::Stepped`, so a `Gui`: the document runs through the
/// real `Runner`, gestures are drive's and answer after the frame that took them, and the
/// pointer stays where the last gesture left it. Its clock advances one frame interval per
/// frame; `settle` never finishes on a page that animates forever.
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

impl Wraps for DocumentTest {}

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
        Self { stepped, fonts }
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }
}
