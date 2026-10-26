use std::rc::Rc;

use beui::dock::styled::DockingLayout;
use beui::styled::PanZoomView;
use beui::reactive::*;

use crate::Fetch;
use crate::images::Images;
use paint_review_manifest::{Frame, Manifest, Painting, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Before,
    After,
    Changes,
    SideBySide,
}

impl Mode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Before => "Before",
            Mode::After => "After",
            Mode::Changes => "Changes",
            Mode::SideBySide => "Side by side",
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Mode::Before => "before",
            Mode::After => "after",
            Mode::Changes => "changes",
            Mode::SideBySide => "side_by_side",
        }
    }

    pub(crate) fn available(self, frame: &Frame) -> bool {
        match self {
            Mode::Before => frame.before.is_some(),
            Mode::After => frame.after.is_some(),
            Mode::Changes => frame.changes.is_some(),
            Mode::SideBySide => frame.before.is_some() && frame.after.is_some(),
        }
    }

    pub(crate) fn images(self, frame: &Frame) -> Vec<String> {
        match self {
            Mode::Before => frame.before.iter().cloned().collect(),
            Mode::After => frame.after.iter().cloned().collect(),
            Mode::Changes => frame.changes.iter().cloned().collect(),
            Mode::SideBySide => frame.before.iter().chain(&frame.after).cloned().collect(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Review {
    pub(crate) manifest: ReadSignal<Fetch<Rc<Manifest>>>,
    pub(crate) images: Images,
    pub(crate) order: Memo<Vec<usize>>,
    pub(crate) selected: Memo<Option<usize>>,
    pub(crate) painting: Memo<Option<Painting>>,
    pub(crate) frame_index: Memo<usize>,
    pub(crate) frame: Memo<Option<Frame>>,
    pub(crate) mode: Memo<Option<Mode>>,
    pub(crate) layout: DockingLayout<u64>,
    pub(crate) camera: Signal<PanZoomView>,
    wanted: Signal<Option<String>>,
    frame_wanted: Signal<usize>,
    mode_wanted: Signal<Mode>,
}

impl Review {
    pub(crate) fn new(
        manifest: ReadSignal<Fetch<Rc<Manifest>>>,
        images: Images,
        initial: Option<String>,
    ) -> Self {
        let wanted = create_signal(initial);
        let frame_wanted = create_signal(0usize);
        let mode_wanted = create_signal(Mode::After);
        let order = create_memo(move || {
            manifest.with(|manifest| match manifest {
                Fetch::Ready(manifest) => {
                    let mut order: Vec<usize> = (0..manifest.paintings.len()).collect();
                    order.sort_by_key(|index| manifest.paintings[*index].status);
                    order
                }
                _ => Vec::new(),
            })
        });
        let selected = create_memo(move || {
            let order = order.get();
            let wanted = wanted.get();
            manifest.with(|manifest| {
                let Fetch::Ready(manifest) = manifest else {
                    return None;
                };
                wanted
                    .and_then(|name| {
                        order
                            .iter()
                            .copied()
                            .find(|index| manifest.paintings[*index].name == name)
                    })
                    .or_else(|| order.first().copied())
            })
        });
        let painting = create_memo(move || {
            let index = selected.get()?;
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
        let mode = create_memo(move || {
            let wanted = mode_wanted.get();
            frame.with(|frame| {
                let frame = frame.as_ref()?;
                if wanted.available(frame) {
                    return Some(wanted);
                }
                [Mode::After, Mode::Before, Mode::Changes]
                    .into_iter()
                    .find(|mode| mode.available(frame))
            })
        });
        let review = Self {
            manifest,
            images,
            order,
            selected,
            painting,
            frame_index,
            frame,
            mode,
            layout: DockingLayout::new(),
            camera: create_signal(PanZoomView::IDENTITY),
            wanted,
            frame_wanted,
            mode_wanted,
        };
        create_effect(move || {
            frame.with(|frame| {
                for hash in frame.iter().flat_map(|frame| {
                    [&frame.before, &frame.after, &frame.changes]
                        .into_iter()
                        .flatten()
                }) {
                    images.want(hash);
                }
            })
        });
        review
    }

    pub(crate) fn names(self, status: Status) -> Vec<(usize, String)> {
        let order = self.order.get();
        self.manifest.with(|manifest| match manifest {
            Fetch::Ready(manifest) => order
                .into_iter()
                .filter(|index| manifest.paintings[*index].status == status)
                .map(|index| (index, manifest.paintings[index].name.clone()))
                .collect(),
            _ => Vec::new(),
        })
    }

    pub(crate) fn select(self, index: usize) {
        let name = self.manifest.with_untracked(|manifest| match manifest {
            Fetch::Ready(manifest) => manifest.paintings.get(index).map(|p| p.name.clone()),
            _ => None,
        });
        if let Some(name) = name {
            self.layout.show(&2);
            self.wanted.set(Some(name));
            self.frame_wanted.set(0);
        }
    }

    pub(crate) fn step_painting(self, by: isize) {
        let order = self.order.get_untracked();
        let Some(at) = self
            .selected
            .get_untracked()
            .and_then(|selected| order.iter().position(|index| *index == selected))
        else {
            return;
        };
        let next = at
            .saturating_add_signed(by)
            .min(order.len().saturating_sub(1));
        if next != at {
            self.select(order[next]);
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

    pub(crate) fn choose(self, mode: Mode) {
        self.mode_wanted.set(mode);
    }

    pub(crate) fn flip(self) {
        let both = self.frame.with_untracked(|frame| {
            frame
                .as_ref()
                .is_some_and(|frame| frame.before.is_some() && frame.after.is_some())
        });
        if !both {
            return;
        }
        let next = match self.mode.get_untracked() {
            Some(Mode::Before) => Mode::After,
            _ => Mode::Before,
        };
        self.mode_wanted.set(next);
    }
}
