use std::cell::RefCell;
use std::rc::Rc;

use beui::native::Context;

use super::Compositor;

#[derive(Default)]
pub(super) struct Clipboard {
    watching: bool,
    copied: Rc<RefCell<Option<String>>>,
    handed: Option<String>,
}

impl Compositor {
    pub fn set_primary_selection(&mut self, on: bool) {
        self.server.state.set_primary_selection(on);
    }

    pub(super) fn sync_clipboard(&mut self, context: &Context) {
        let clipboard = &mut self.clipboard;
        if !clipboard.watching {
            clipboard.watching = true;
            let copied = clipboard.copied.clone();
            let waker = self.waker.clone();
            context.watch_copies(move |text| {
                *copied.borrow_mut() = Some(text.to_owned());
                if let Some(waker) = &waker {
                    waker.wake();
                }
            });
        }
        let copied = clipboard.copied.borrow_mut().take();
        if let Some(text) = copied {
            match clipboard.handed.take() {
                Some(handed) if handed == text => {}
                _ => self.server.state.offer_selection_text(text),
            }
        }
        if let Some(text) = self.server.state.take_selection_text() {
            self.clipboard.handed = Some(text.clone());
            context.copy_text(text);
        }
    }
}
