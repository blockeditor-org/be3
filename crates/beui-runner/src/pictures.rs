use std::cell::RefCell;
use std::rc::Rc;

use beui_core::context::Context;
use beui_types::Image;

use crate::renderer::Capture;

type Answer = Box<dyn FnOnce(Option<Image>)>;

/// The window as it was last presented, for whatever asks on the window's thread
/// (be-host's screenshot portal when it runs in a window). The runner provides
/// one in the `Setup`; an answer comes at the end of the next frame, `None` while
/// nothing has been presented or the renderer cannot read the frame back.
#[derive(Clone)]
pub struct WindowPictures(Rc<Inner>);

struct Inner {
    context: Context,
    asked: RefCell<Vec<Answer>>,
}

impl WindowPictures {
    pub(crate) fn new(context: Context) -> Self {
        Self(Rc::new(Inner {
            context,
            asked: RefCell::default(),
        }))
    }

    pub fn take(&self, answer: impl FnOnce(Option<Image>) + 'static) {
        self.0.asked.borrow_mut().push(Box::new(answer));
        self.0.context.request_repaint();
    }

    pub(crate) fn asked(&self) -> bool {
        !self.0.asked.borrow().is_empty()
    }

    pub(crate) fn answer(&self, capture: Option<Capture>) {
        let answers = std::mem::take(&mut *self.0.asked.borrow_mut());
        let picture =
            capture.map(|capture| Image::from_rgba(capture.width, capture.height, capture.rgba));
        for answer in answers {
            answer(picture.clone());
        }
    }
}
