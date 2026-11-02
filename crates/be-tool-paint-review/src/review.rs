use std::rc::Rc;

use beui::reactive::*;
use beui::styled::PanZoomView;

use crate::Fetch;
use crate::images::Images;
use be_tool_paint_review_manifest::{Frame, Manifest, Painting, Picture, Status};

const NEAR: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Side {
    Before,
    After,
    Changes,
}

impl Side {
    pub(crate) const ALL: [Side; 3] = [Side::After, Side::Before, Side::Changes];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Side::Before => "Before",
            Side::After => "After",
            Side::Changes => "Changes",
        }
    }

    pub(crate) fn of(self, frame: &Frame) -> Option<&Picture> {
        match self {
            Side::Before => frame.before.as_ref(),
            Side::After => frame.after.as_ref(),
            Side::Changes => frame.changes.as_ref(),
        }
    }
}

pub(crate) fn missing(status: Status, side: Side) -> &'static str {
    match (status, side) {
        (Status::Unreadable, _) => "Unreadable",
        (Status::Added, _) => "Added",
        (Status::Removed, _) => "Removed",
        (Status::Changed, Side::Before) => "Not drawn before",
        (Status::Changed, _) => "Not drawn after",
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Review {
    pub(crate) manifest: ReadSignal<Fetch<Rc<Manifest>>>,
    pub(crate) images: Images,
    pub(crate) count: Memo<usize>,
    pub(crate) open: Memo<Option<usize>>,
    pub(crate) hovered: Signal<Option<usize>>,
    pub(crate) columns: Signal<usize>,
    pub(crate) space: Signal<bool>,
    pub(crate) side: Memo<Side>,
    pub(crate) holding: Memo<bool>,
    pub(crate) painting: Memo<Option<Painting>>,
    pub(crate) frame_index: Memo<usize>,
    pub(crate) frame: Memo<Option<Frame>>,
    pub(crate) split: Signal<f32>,
    pub(crate) camera: Signal<PanZoomView>,
    opened: Signal<Option<String>>,
    frame_wanted: Signal<usize>,
}

impl Review {
    pub(crate) fn new(
        manifest: ReadSignal<Fetch<Rc<Manifest>>>,
        images: Images,
        initial: Option<String>,
    ) -> Self {
        let opened = create_signal(initial);
        let frame_wanted = create_signal(0usize);
        let space = create_signal(false);
        let modifiers = held_modifiers();
        let shift = create_memo(move || modifiers.get().shift);
        let side = create_memo(move || match (shift.get(), space.get()) {
            (true, _) => Side::Changes,
            (false, true) => Side::Before,
            (false, false) => Side::After,
        });
        let holding = create_memo(move || shift.get() || space.get());
        let count = create_memo(move || {
            manifest.with(|manifest| match manifest {
                Fetch::Ready(manifest) => manifest.paintings.len(),
                _ => 0,
            })
        });
        let open = create_memo(move || {
            let name = opened.get()?;
            manifest.with(|manifest| match manifest {
                Fetch::Ready(manifest) => manifest
                    .paintings
                    .iter()
                    .position(|painting| painting.name == name),
                _ => None,
            })
        });
        let painting = create_memo(move || {
            let index = open.get()?;
            manifest.with(|manifest| match manifest {
                Fetch::Ready(manifest) => manifest.paintings.get(index).cloned(),
                _ => None,
            })
        });
        let frame_index = create_memo(move || {
            let count = painting.with(|painting| painting.as_ref().map_or(0, |p| p.frames.len()));
            frame_wanted.get().min(count.saturating_sub(1))
        });
        let frame = create_memo(move || {
            let index = frame_index.get();
            painting.with(|painting| painting.as_ref()?.frames.get(index).cloned())
        });
        let review = Self {
            manifest,
            images,
            count,
            open,
            hovered: create_signal(None),
            columns: create_signal(1),
            space,
            side,
            holding,
            painting,
            frame_index,
            frame,
            split: create_signal(0.5),
            camera: create_signal(PanZoomView::IDENTITY),
            opened,
            frame_wanted,
        };
        create_effect(move || {
            manifest.with(|manifest| {
                let Fetch::Ready(manifest) = manifest else {
                    return;
                };
                for side in Side::ALL {
                    for painting in &manifest.paintings {
                        if let Some(picture) = painting.frames.first().and_then(|f| side.of(f)) {
                            images.want(&picture.thumbnail);
                        }
                    }
                }
            })
        });
        create_effect(move || {
            frame.with(|frame| {
                for side in Side::ALL {
                    if let Some(picture) = frame.as_ref().and_then(|frame| side.of(frame)) {
                        images.want(&picture.image);
                    }
                }
            })
        });
        create_effect(move || {
            if open.get().is_some() {
                return;
            }
            let Some(hovered) = review.hovered.get() else {
                return;
            };
            let shown = side.get();
            let columns = review.columns.get().max(1);
            review.prefetch(hovered, &Side::ALL);
            for index in nearby(hovered, columns, count.get_untracked()) {
                review.prefetch(index, &[shown]);
            }
        });
        review
    }

    fn prefetch(self, index: usize, sides: &[Side]) {
        self.manifest.with_untracked(|manifest| {
            let Fetch::Ready(manifest) = manifest else {
                return;
            };
            let Some(frame) = manifest.paintings.get(index).and_then(|p| p.frames.first()) else {
                return;
            };
            for side in sides {
                if let Some(picture) = side.of(frame) {
                    self.images.want(&picture.image);
                }
            }
        });
    }

    pub(crate) fn with_painting<T>(self, index: usize, read: impl FnOnce(&Painting) -> T) -> Option<T> {
        self.manifest.with(|manifest| match manifest {
            Fetch::Ready(manifest) => manifest.paintings.get(index).map(read),
            _ => None,
        })
    }

    pub(crate) fn select(self, index: usize) {
        let name = self.manifest.with_untracked(|manifest| match manifest {
            Fetch::Ready(manifest) => manifest.paintings.get(index).map(|p| p.name.clone()),
            _ => None,
        });
        if let Some(name) = name {
            self.opened.set(Some(name));
            self.frame_wanted.set(0);
            self.split.set(0.5);
        }
    }

    pub(crate) fn close(self) {
        self.opened.set(None);
    }

    pub(crate) fn step_painting(self, by: isize) {
        let count = self.count.get_untracked();
        let Some(at) = self.open.get_untracked() else {
            return;
        };
        let next = at.saturating_add_signed(by).min(count.saturating_sub(1));
        if next != at {
            self.select(next);
        }
    }

    pub(crate) fn frame_count(self) -> usize {
        self.painting.with(|painting| {
            painting
                .as_ref()
                .map_or(0, |painting| painting.frames.len())
        })
    }

    pub(crate) fn step_frame(self, by: isize) {
        let count = self.painting.with_untracked(|painting| {
            painting
                .as_ref()
                .map_or(0, |painting| painting.frames.len())
        });
        let at = self.frame_index.get_untracked();
        let next = at.saturating_add_signed(by).min(count.saturating_sub(1));
        self.frame_wanted.set(next);
    }
}

pub(crate) fn nearby(index: usize, columns: usize, count: usize) -> Vec<usize> {
    let (row, column) = ((index / columns) as isize, (index % columns) as isize);
    let near = NEAR as isize;
    let mut found = Vec::new();
    for dy in -1..=1isize {
        for dx in -near..=near {
            if (dx, dy) == (0, 0) || (dy != 0 && dx.abs() > 1) {
                continue;
            }
            let (r, c) = (row + dy, column + dx);
            if r < 0 || c < 0 || c >= columns as isize {
                continue;
            }
            let at = (r * columns as isize + c) as usize;
            if at < count {
                found.push(at);
            }
        }
    }
    found
}
