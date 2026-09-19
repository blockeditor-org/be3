use std::cell::RefCell;
use std::rc::Rc;

use block_client::blocks::image::{Image as ImageBlock, ImageMetadata, ImageOperation};
use block_editor_plugin::beui::Image;
use block_editor_plugin::beui::reactive::{Memo, create_memo, create_signal};
use block_editor_plugin::{BlockProjection, Editor};

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Shown {
    pub(crate) image: Option<Image>,
    pub(crate) error: Option<String>,
}

#[derive(Default)]
struct Decoded {
    revision: Option<u64>,
    shown: Shown,
}

pub(crate) fn watch(editor: &Editor, block: &Rc<BlockProjection<ImageBlock>>) -> Memo<Shown> {
    let (shown, set_shown) = create_signal(Shown::default());
    let decoded = RefCell::new(Decoded::default());
    let handle = block.handle().clone();
    let operating = Rc::clone(block);
    let editable = editor.editable();
    editor.each_frame(move || {
        let revision = handle.revision();
        let mut decoded = decoded.borrow_mut();
        if decoded.revision == Some(revision) {
            set_shown.set(decoded.shown.clone());
            return;
        }
        let Some(image) = handle.read() else {
            return;
        };
        decoded.revision = Some(revision);
        if let ImageMetadata::Failed(error) = image.metadata() {
            decoded.shown = Shown {
                image: None,
                error: Some(error.clone()),
            };
            set_shown.set(decoded.shown.clone());
            return;
        }
        let recorded = image.metadata().clone();
        let result = crate::decode::decode(image.data());
        drop(image);
        let may_write = editable.get_untracked();
        decoded.shown = match result {
            Ok(found) => {
                if may_write && recorded != found.metadata {
                    operating.operate(ImageOperation::SetMetadata {
                        metadata: found.metadata,
                    });
                }
                Shown {
                    image: Some(Image::from_rgba(found.width, found.height, found.pixels)),
                    error: None,
                }
            }
            Err(error) => {
                if may_write {
                    operating.operate(ImageOperation::SetMetadata {
                        metadata: ImageMetadata::Failed(error.clone()),
                    });
                }
                Shown {
                    image: None,
                    error: Some(error),
                }
            }
        };
        set_shown.set(decoded.shown.clone());
    });
    create_memo(move || shown.get())
}
