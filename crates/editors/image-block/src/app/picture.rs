use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Image, Thumbhash};
use block_editor_beui::{BlockQuery, ContentProjection, Editor};
use image_block_types::{ImageContent, ImageHeader, ImageOp};

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

pub(crate) fn watch(editor: Editor, block: ContentProjection<ImageContent>) -> Memo<Shown> {
    let shown = create_signal(Shown::default());
    let key = block.project(|image| (image.header().clone(), digest(image.data())));
    let decoded: RefCell<Option<(u64, Shown)>> = RefCell::new(None);
    let editable = editor.editable();
    create_effect(move || {
        let (header, fingerprint) = key.get();
        if let Some(failure) = &header.failure {
            shown.set(Shown {
                image: None,
                error: Some(failure.clone()),
            });
            return;
        }
        let mut decoded = decoded.borrow_mut();
        if let Some((held, cached)) = decoded.as_ref()
            && *held == fingerprint
        {
            shown.set(cached.clone());
            return;
        }
        let Ok(result) = block.read(|image| crate::decode::decode(image.data())) else {
            return;
        };
        let (found, decoded_shown) = match result {
            Ok(found) => {
                let image = Image::from_rgba(found.width, found.height, found.pixels);
                let recorded = ImageHeader {
                    media_type: found.media_type,
                    width: found.width,
                    height: found.height,
                    failure: None,
                    thumbhash: Some(image.thumbhash().hash),
                    ..header.clone()
                };
                let shown = Shown {
                    image: Some(image),
                    error: None,
                };
                (recorded, shown)
            }
            Err(error) => (
                ImageHeader {
                    failure: Some(error.clone()),
                    thumbhash: None,
                    ..header.clone()
                },
                Shown {
                    image: None,
                    error: Some(error),
                },
            ),
        };
        *decoded = Some((fingerprint, decoded_shown.clone()));
        drop(decoded);
        shown.set(decoded_shown);
        if editable.get_untracked() && found != header {
            block.operate(ImageOp::SetHeader(found));
        }
    });
    create_memo(move || shown.get())
}

pub(crate) fn thumbhash(editor: Editor) -> Memo<Option<Thumbhash>> {
    let info = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
    create_memo(move || {
        info.get()
            .ok()?
            .into_iter()
            .next()
            .and_then(|info| info.thumbhash())
            .map(|thumbhash| Thumbhash {
                hash: thumbhash.hash,
                width: thumbhash.width,
                height: thumbhash.height,
            })
    })
}
