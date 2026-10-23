use beui_components_styled::{SoftwareKeyboard, keyboard_height};
use beui_core::context::Context;
use beui_core::document::Document;
use beui_core::geometry::Rect;
use beui_core::software_keyboard::KeyboardHost;
use beui_macros::view;
use beui_view::reactive::enter;

#[derive(Default)]
struct KeyboardWindow {
    document: Option<Document>,
}

impl KeyboardHost for KeyboardWindow {
    fn height(&mut self, width: f32, scale: f32) -> f32 {
        keyboard_height(width, scale)
    }

    fn show(&mut self, ctx: &Context, rect: Rect) {
        let document = self.document.get_or_insert_with(|| {
            let mut document = Document::new();
            let root = enter(&mut document, || {
                view! {
                    <SoftwareKeyboard />
                }
            });
            document.set_root(root);
            document
        });
        document.show_content(ctx, rect);
    }

    fn closed(&mut self) {
        self.document = None;
    }
}

pub(crate) fn install_keyboard(document: &mut Document) {
    document.set_keyboard_host(Box::new(KeyboardWindow::default()));
}
