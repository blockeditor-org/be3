use beui_core::context::Context;
use beui_core::document::Document;
use beui_core::geometry::{Rect, Vec2, vec2};
use beui_core::software_keyboard::KeyboardHost;
use beui_core::window::{WindowId, WindowRole};
use beui_keyboard::styled::SoftwareKeyboard;
use beui_macros::view;
use beui_view::components::window::{Window, WindowHandle};
use beui_view::reactive::enter;

pub const WINDOW_SIZE: Vec2 = vec2(760.0, 280.0);

#[derive(Default)]
struct KeyboardWindow {
    document: Option<(Document, Option<WindowId>)>,
}

impl KeyboardHost for KeyboardWindow {
    fn show(&mut self, ctx: &Context, rect: Rect, window: Option<WindowId>) {
        if self
            .document
            .as_ref()
            .is_some_and(|(_, shown)| *shown != window)
        {
            self.document = None;
        }
        let (document, _) = self.document.get_or_insert_with(|| {
            let mut document = Document::new();
            let root = enter(&mut document, || match window {
                Some(id) => view! {
                    <Window
                        id={id}
                        size={WINDOW_SIZE}
                        role={WindowRole::Keyboard}
                        title={Some("Keyboard".to_owned())}
                    >|_: WindowHandle|
                        <SoftwareKeyboard />
                    </Window>
                },
                None => view! {
                    <SoftwareKeyboard />
                },
            });
            document.set_root(root);
            (document, window)
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
