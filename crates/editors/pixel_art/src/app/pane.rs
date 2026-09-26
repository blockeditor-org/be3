use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::pixel_art::Artwork;
use block_editor_beui::be_block::pixel_art::PixelColor;
use block_editor_beui::beui::Image;
use block_editor_beui::beui::reactive::{Memo, create_memo, create_signal};

use super::state::{ArtBlock, artwork_of};

use crate::color::{artwork_image, preview_bounds, preview_image};

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Shown {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) artwork: Option<Image>,
    pub(crate) preview: Option<(u16, u16, u16, u16, Image)>,
}

#[derive(Default)]
struct Held {
    revision: Option<u64>,
    dark_mode: bool,
    size: (u16, u16),
    pixels: Vec<(u16, u16)>,
    color: Option<PixelColor>,
    shown: Shown,
}

pub(crate) struct Pane {
    held: RefCell<Held>,
    art: RefCell<Option<(u64, Rc<Artwork>)>>,
    shown: Memo<Shown>,
    set_shown: block_editor_beui::beui::reactive::WriteSignal<Shown>,
}

impl Pane {
    pub(crate) fn new() -> Rc<Self> {
        let (shown, set_shown) = create_signal(Shown::default());
        Rc::new(Self {
            held: RefCell::new(Held {
                dark_mode: true,
                ..Held::default()
            }),
            art: RefCell::new(None),
            shown: create_memo(move || shown.get()),
            set_shown,
        })
    }

    pub(crate) fn shown(&self) -> Memo<Shown> {
        self.shown.clone()
    }

    pub(crate) fn refresh(
        &self,
        block: &ArtBlock,
        dark_mode: bool,
        pixels: &[(u16, u16)],
        color: PixelColor,
    ) {
        let Some(revision) = block.revision() else {
            return;
        };
        let mut held = self.held.borrow_mut();
        let Some(art) = artwork_of(block, &self.art) else {
            return;
        };
        let size = (art.width(), art.height());
        let stale = held.revision != Some(revision) || held.dark_mode != dark_mode;
        if stale {
            held.shown.artwork = Some(artwork_image(&art, dark_mode));
            held.shown.width = size.0;
            held.shown.height = size.1;
            held.revision = Some(revision);
            held.dark_mode = dark_mode;
            held.size = size;
            held.pixels.clear();
            held.color = None;
        }
        drop(art);
        let same =
            held.pixels == pixels && (pixels.is_empty() || held.color == Some(color)) && !stale;
        if !same {
            held.shown.preview = preview_bounds(pixels).map(|bounds| {
                let (left, top, right, bottom) = bounds;
                (
                    left,
                    top,
                    right,
                    bottom,
                    preview_image(pixels, color, bounds, dark_mode),
                )
            });
            held.pixels = pixels.to_vec();
            held.color = (!pixels.is_empty()).then_some(color);
        }
        let shown = held.shown.clone();
        drop(held);
        self.set_shown.set(shown);
    }
}
