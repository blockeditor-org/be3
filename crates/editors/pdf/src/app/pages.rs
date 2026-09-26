use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::PdfContent;
use block_editor_beui::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_beui::beui::{Image, Rect, Vec2};
use block_editor_beui::{ContentProjection, PerformanceReporter, Waker};

use crate::pane::Pane;

use super::DEFAULT_PAGE_SIZE;

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Shown {
    pub(crate) tiles: Vec<(Rect, Image)>,
    pub(crate) page_size: Option<Vec2>,
    pub(crate) page_count: Option<usize>,
    pub(crate) page: usize,
    pub(crate) error: Option<String>,
}

impl Shown {
    pub(crate) fn size(&self) -> Vec2 {
        self.page_size.unwrap_or(DEFAULT_PAGE_SIZE)
    }
}

pub(crate) struct Pages {
    pane: RefCell<Pane>,
    page: ReadSignal<usize>,
    set_page: WriteSignal<usize>,
    shown: ReadSignal<Shown>,
    set_shown: WriteSignal<Shown>,
}

pub(crate) struct Viewport {
    pub(crate) page_rect: Rect,
    pub(crate) visible: Rect,
    pub(crate) pixels_per_point: f32,
}

impl Pages {
    pub(crate) fn new() -> Rc<Self> {
        let (shown, set_shown) = create_signal(Shown::default());
        let (page, set_page) = create_signal(0);
        Rc::new(Self {
            pane: RefCell::new(Pane::default()),
            page,
            set_page,
            shown,
            set_shown,
        })
    }

    pub(crate) fn shown(&self) -> Memo<Shown> {
        let shown = self.shown.clone();
        create_memo(move || shown.get())
    }

    pub(crate) fn page(&self) -> usize {
        self.page.get_untracked()
    }

    pub(crate) fn go(&self, page: usize) {
        self.set_page.set(page);
    }

    pub(crate) fn pump(
        &self,
        block: &ContentProjection<PdfContent>,
        performance: &PerformanceReporter,
        waker: Waker,
        viewport: Viewport,
    ) {
        let mut pane = self.pane.borrow_mut();
        let mut shown = self.shown.get_untracked();
        if let Some(facts) = pane.poll(performance) {
            shown.page_count = Some(facts.page_count);
            if self.page.get_untracked() >= facts.page_count {
                self.set_page.set(facts.page_index);
            }
            shown.page_size = Some(facts.page_size_pts);
        }
        let Some(revision) = block.revision() else {
            return;
        };
        pane.ensure(
            revision,
            self.page.get(),
            shown.page_size,
            viewport.page_rect,
            viewport.visible,
            viewport.pixels_per_point,
            waker,
            performance,
            || block.read(|pdf| pdf.data().to_vec()),
        );
        shown.tiles = pane.tiles();
        shown.page = self.page.get_untracked();
        shown.error = pane.error().map(str::to_owned);
        drop(pane);
        self.set_shown.set(shown);
    }
}
