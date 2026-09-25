use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use block_editor_plugin::be_block::{ImageContent, ImageHeader, ImageOp};
use block_editor_plugin::beui::Image;
use block_editor_plugin::beui::reactive::{Memo, create_effect, create_memo, create_signal};
use block_editor_plugin::{ContentProjection, Editor};

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Shown {
    pub(crate) image: Option<Image>,
    pub(crate) error: Option<String>,
}

fn digest(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn watch(editor: &Editor, block: &Rc<ContentProjection<ImageContent>>) -> Memo<Shown> {
    let (shown, set_shown) = create_signal(Shown::default());
    let key = block.project(|image| (image.header().clone(), digest(image.data())));
    let decoded: RefCell<Option<(u64, Shown)>> = RefCell::new(None);
    let reading = Rc::clone(block);
    let editable = editor.editable();
    create_effect(move || {
        let (header, fingerprint) = key.get();
        if let Some(failure) = &header.failure {
            set_shown.set(Shown {
                image: None,
                error: Some(failure.clone()),
            });
            return;
        }
        let mut decoded = decoded.borrow_mut();
        if let Some((held, shown)) = decoded.as_ref()
            && *held == fingerprint
        {
            set_shown.set(shown.clone());
            return;
        }
        let Some(result) = reading.read(|image| crate::decode::decode(image.data())) else {
            return;
        };
        let (found, shown) = match result {
            Ok(found) => {
                let shown = Shown {
                    image: Some(Image::from_rgba(found.width, found.height, found.pixels)),
                    error: None,
                };
                let recorded = ImageHeader {
                    media_type: found.media_type,
                    width: found.width,
                    height: found.height,
                    failure: None,
                    ..header.clone()
                };
                (recorded, shown)
            }
            Err(error) => (
                ImageHeader {
                    failure: Some(error.clone()),
                    ..header.clone()
                },
                Shown {
                    image: None,
                    error: Some(error),
                },
            ),
        };
        *decoded = Some((fingerprint, shown.clone()));
        drop(decoded);
        set_shown.set(shown);
        if editable.get_untracked() && found != header {
            reading.operate(ImageOp::SetHeader(found));
        }
    });
    create_memo(move || shown.get())
}
