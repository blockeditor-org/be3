use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use block_editor_beui::BlockParent;
use block_editor_beui::be_block::paint::{ApprovedPainting, PaintReview};
use block_editor_beui::be_block::{PaintReviewContent, PaintSnapshotContent, PaintSnapshotHeader};
use block_editor_beui::beui::reactive::{
    ReadSignal, Stored, WriteSignal, create_effect, create_signal, store, untrack,
};
use block_editor_beui::{ContentProjection, Editor, Waker};

use crate::download::{BRANCH, Download, Painting, Source};
use crate::render::{Change, Paintings, Rendered};

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Showing {
    Approved,
    #[default]
    Current,
    Difference,
    SideBySide,
}

impl Showing {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Approved => "the painting you approved".to_owned(),
            Self::Current => format!("the painting on the {BRANCH} branch"),
            Self::Difference => "the pixels that changed".to_owned(),
            Self::SideBySide => format!("the painting you approved, beside the one on {BRANCH}"),
        }
    }

    fn wanted(self) -> [Self; 2] {
        match self {
            Self::Approved => [Self::Approved, Self::Current],
            _ => [Self::Current, Self::Approved],
        }
    }

    fn both(self) -> bool {
        matches!(self, Self::Difference | Self::SideBySide)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Status {
    New,
    Modified,
    Removed,
    Unchanged,
}

impl Status {
    pub(crate) const ALL: [Self; 4] = [Self::New, Self::Modified, Self::Removed, Self::Unchanged];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Modified => "Modified",
            Self::Removed => "Removed",
            Self::Unchanged => "Approved",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub status: Status,
}

#[derive(Clone, PartialEq)]
pub(crate) enum Shown {
    Ready(Vec<Rendered>, String),
    Failed(String),
    Waiting(Option<String>),
}

#[derive(Clone, Copy)]
pub(crate) struct Review {
    editor: Editor,
    loaded: ReadSignal<bool>,
    pub(crate) found: ReadSignal<Vec<Painting>>,
    set_found: WriteSignal<Vec<Painting>>,
    pub(crate) approved: ReadSignal<Vec<ApprovedPainting>>,
    pub(crate) error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    pub(crate) selected: ReadSignal<Option<String>>,
    set_selected: WriteSignal<Option<String>>,
    pub(crate) showing: ReadSignal<Showing>,
    set_showing: WriteSignal<Showing>,
    pub(crate) pending: ReadSignal<Option<String>>,
    set_pending: WriteSignal<Option<String>>,
    pub(crate) frame: ReadSignal<usize>,
    set_frame: WriteSignal<usize>,
    pub(crate) playing: ReadSignal<bool>,
    set_playing: WriteSignal<bool>,
    pub(crate) downloading: ReadSignal<bool>,
    set_downloading: WriteSignal<bool>,
    pub(crate) scale: ReadSignal<Option<f32>>,
    set_scale: WriteSignal<Option<f32>>,
    pub(crate) description: ReadSignal<Option<String>>,
    set_description: WriteSignal<Option<String>>,
    pub(crate) revision: ReadSignal<u64>,
    set_revision: WriteSignal<u64>,
    inner: Stored<Inner>,
}

struct Inner {
    block: ContentProjection<PaintReviewContent>,
    source: Source,
    download: RefCell<Option<Download>>,
    waker: RefCell<Waker>,
    paintings: RefCell<Paintings>,
    change: RefCell<Option<(String, Change)>>,
    advanced: Cell<Option<Instant>>,
}

impl Review {
    pub(crate) fn new(editor: Editor, source: Source) -> Self {
        let block = editor.block_content::<PaintReviewContent>();
        let approved = block.project(|review| review.root().approved());
        let loaded = block.loaded();
        let (found, set_found) = create_signal(Vec::new());
        let (error, set_error) = create_signal(None);
        let (selected, set_selected) = create_signal(None);
        let (showing, set_showing) = create_signal(Showing::default());
        let (pending, set_pending) = create_signal(None);
        let (frame, set_frame) = create_signal(0);
        let (playing, set_playing) = create_signal(false);
        let (downloading, set_downloading) = create_signal(false);
        let (scale, set_scale) = create_signal(None);
        let (description, set_description) = create_signal(None);
        let (revision, set_revision) = create_signal(0);
        Self {
            editor,
            loaded,
            found,
            set_found,
            approved,
            error,
            set_error,
            selected,
            set_selected,
            showing,
            set_showing,
            pending,
            set_pending,
            frame,
            set_frame,
            playing,
            set_playing,
            downloading,
            set_downloading,
            scale,
            set_scale,
            description,
            set_description,
            revision,
            set_revision,
            inner: store(Inner {
                block,
                source,
                download: RefCell::new(None),
                waker: RefCell::new(Waker::default()),
                paintings: RefCell::new(Paintings::default()),
                change: RefCell::new(None),
                advanced: Cell::new(None),
            }),
        }
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn editable(&self) -> bool {
        self.editor.host().editable()
    }

    pub(crate) fn rastered(&self) -> usize {
        let inner = self.inner.get();
        inner.paintings.borrow().rastered()
    }

    pub(crate) fn watch(self) {
        let (waker, woken) = self.editor.woken();
        self.inner.with(|inner| *inner.waker.borrow_mut() = waker);
        create_effect(move || {
            woken.with(|_| ());
            untrack(|| self.raster());
        });
        self.editor.on_reply(move || {
            if self.inner.is_alive() {
                self.fetch();
            }
        });
        let editable = self.editor.editable();
        create_effect(move || {
            self.pending.with(|_| ());
            self.found.with(|_| ());
            editable.with(|_| ());
            self.inner.with(|inner| inner.block.revision());
            untrack(|| self.settle());
        });
        create_effect(move || {
            let selected = self.selected.get();
            self.showing.with(|_| ());
            self.approved.with(|_| ());
            self.found.with(|_| ());
            match selected {
                Some(path) => self.request(&path),
                None => self.forget(),
            }
            untrack(|| self.raster());
        });
        self.fetch();
    }

    fn fetch(&self) {
        let inner = self.inner.get();
        if inner.download.borrow().is_none() {
            *inner.download.borrow_mut() =
                Some(crate::download::start(&inner.source, &self.editor.host()));
        }
        let result = inner
            .download
            .borrow_mut()
            .as_mut()
            .and_then(|download| download.poll(&self.editor.host()));
        if let Some(result) = result {
            match result {
                Ok(found) => {
                    self.set_found.set(found);
                    self.set_error.set(None);
                }
                Err(error) => {
                    self.set_found.set(Vec::new());
                    self.set_error.set(Some(error));
                }
            }
        }
        let finished = inner
            .download
            .borrow()
            .as_ref()
            .is_none_or(Download::finished);
        self.set_downloading.set(!finished);
    }

    pub(crate) fn raster(&self) {
        let inner = self.inner.get();
        let waker = inner.waker.borrow().clone();
        let changed = inner.paintings.borrow_mut().settle(&waker);
        if changed {
            self.bump();
        }
    }

    fn bump(&self) {
        self.set_revision.update(|revision| *revision += 1);
    }

    pub(crate) fn refresh(&self) {
        let inner = self.inner.get();
        inner.download.borrow_mut().take();
        self.fetch();
    }

    pub(crate) fn entries(&self) -> Option<Vec<Entry>> {
        if !self.loaded.get() {
            return None;
        }
        let approvals = self.approved.get();
        let found = self.found.get();
        let mut entries: Vec<Entry> = found
            .iter()
            .map(|painting| Entry {
                path: painting.path.clone(),
                status: match approvals
                    .iter()
                    .find(|approved| approved.path == painting.path)
                {
                    None => Status::New,
                    Some(approved) if approved.hash != painting.hash => Status::Modified,
                    Some(_) => Status::Unchanged,
                },
            })
            .collect();
        entries.extend(
            approvals
                .iter()
                .filter(|approved| !found.iter().any(|painting| painting.path == approved.path))
                .map(|approved| Entry {
                    path: approved.path.clone(),
                    status: Status::Removed,
                }),
        );
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Some(entries)
    }

    pub(crate) fn status(&self, path: &str) -> Option<Status> {
        self.entries()?
            .into_iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.status)
    }

    pub(crate) fn shown_as(&self, status: Status) -> Showing {
        match status {
            Status::Removed => Showing::Approved,
            Status::New | Status::Unchanged => Showing::Current,
            Status::Modified => self.showing.get(),
        }
    }

    fn approval(&self, path: &str) -> Option<ApprovedPainting> {
        self.approved
            .get_untracked()
            .into_iter()
            .find(|approved| approved.path == path)
    }

    pub(crate) fn select(&self, path: &str) {
        let inner = self.inner.get();
        self.set_selected.set(Some(path.to_owned()));
        self.set_showing.set(Showing::Current);
        self.set_frame.set(0);
        self.set_playing.set(false);
        inner.advanced.set(None);
        self.editor.fit();
    }

    pub(crate) fn show(&self, showing: Showing) {
        self.set_showing.set(showing);
    }

    pub(crate) fn approve(&self, path: &str) {
        self.set_showing.set(Showing::Current);
        self.set_pending.set(Some(path.to_owned()));
        self.settle();
    }

    fn settle(&self) {
        let Some(path) = self.pending.get_untracked() else {
            return;
        };
        if self.approve_now(&path) {
            self.set_pending.set(None);
        }
    }

    fn approve_now(&self, path: &str) -> bool {
        let inner = self.inner.get();
        if !self.editable() {
            return true;
        }
        let found = self.found.get_untracked();
        let Some(painting) = found.iter().find(|found| found.path == path) else {
            return true;
        };
        let Some(approved) = inner.block.read(|review| {
            review
                .root()
                .approval(path)
                .map(|approved| approved.snapshot)
        }) else {
            return false;
        };
        let snapshot = PaintSnapshotContent::new(
            PaintSnapshotHeader {
                path: path.to_owned(),
                hash: painting.hash.clone(),
            },
            painting.data.clone(),
        );
        let reference = match approved {
            Some(id) => {
                self.editor.replace_content(id, &snapshot);
                id
            }
            None => self.editor.create_child(&snapshot),
        };
        inner
            .block
            .operate(PaintReview::approve(path, painting.hash.clone(), reference));
        true
    }

    pub(crate) fn unapprove(&self, path: &str) {
        let inner = self.inner.get();
        if self.pending.get_untracked().as_deref() == Some(path) {
            self.set_pending.set(None);
        }
        self.set_showing.set(Showing::Current);
        if !self.editable() {
            return;
        }
        let Some(approval) = self.approval(path) else {
            return;
        };
        self.editor
            .blocks()
            .set_parent(approval.snapshot, BlockParent::Detached);
        inner.block.operate(PaintReview::forget(path));
    }

    fn hash(&self, path: &str, showing: Showing) -> Result<String, Option<String>> {
        match showing {
            Showing::Approved => self
                .approval(path)
                .map(|approved| approved.hash)
                .ok_or_else(|| Some(format!("{path} has never been approved"))),
            _ => self
                .found
                .get_untracked()
                .iter()
                .find(|found| found.path == path)
                .map(|painting| painting.hash.clone())
                .ok_or_else(|| Some(format!("{path} is not on the {BRANCH} branch"))),
        }
    }

    fn data(&self, path: &str, showing: Showing) -> Result<Vec<u8>, Option<String>> {
        match showing {
            Showing::Approved => {
                let approval = self
                    .approval(path)
                    .ok_or_else(|| Some(format!("{path} has never been approved")))?;
                let id = Some(approval.snapshot).ok_or_else(|| {
                    Some("the approved painting is not on this workspace".to_owned())
                })?;
                self.editor
                    .content_of::<PaintSnapshotContent>(id)
                    .read(|snapshot| snapshot.data().to_vec())
                    .ok_or(None)
            }
            _ => self
                .found
                .get_untracked()
                .iter()
                .find(|found| found.path == path)
                .map(|painting| painting.data.clone())
                .ok_or_else(|| Some(format!("{path} is not on the {BRANCH} branch"))),
        }
    }

    pub(crate) fn request(&self, path: &str) {
        let inner = self.inner.get();
        let mut kept = vec![self.difference_key(path)];
        for wanted in self.showing.get_untracked().wanted() {
            let Ok(hash) = self.hash(path, wanted) else {
                continue;
            };
            kept.push(hash.clone());
            if inner.paintings.borrow().holds(&hash) {
                continue;
            }
            let Ok(data) = self.data(path, wanted) else {
                continue;
            };
            inner.paintings.borrow_mut().want(&hash, data);
        }
        inner.paintings.borrow_mut().keep(kept);
    }

    pub(crate) fn forget(&self) {
        let inner = self.inner.get();
        inner.paintings.borrow_mut().keep(Vec::new());
    }

    fn difference_key(&self, path: &str) -> String {
        format!(
            "\u{1}difference\u{1}{}\u{1}{}",
            self.hash(path, Showing::Approved).unwrap_or_default(),
            self.hash(path, Showing::Current).unwrap_or_default(),
        )
    }

    pub(crate) fn count(&self, path: &str, showing: Showing) -> usize {
        let inner = self.inner.get();
        let paintings = inner.paintings.borrow();
        let counts = showing
            .wanted()
            .into_iter()
            .filter_map(|wanted| self.hash(path, wanted).ok())
            .filter_map(|hash| paintings.count(&hash));
        let count = match showing.both() {
            true => counts.min(),
            false => counts.take(1).next(),
        };
        count.unwrap_or(1).max(1)
    }

    pub(crate) fn rastering(&self) -> bool {
        let inner = self.inner.get();
        inner.paintings.borrow().working()
    }

    pub(crate) fn loading(&self) -> Option<(usize, usize)> {
        let inner = self.inner.get();
        let path = self.selected.get()?;
        let status = self.status(&path)?;
        let hash = self.hash(&path, self.shown_as(status)).ok()?;
        inner.paintings.borrow().loading(&hash)
    }

    pub(crate) fn changed(&self, path: &str, status: Status) -> Option<(String, Option<usize>)> {
        if status != Status::Modified {
            return None;
        }
        self.change(path)
    }

    fn change(&self, path: &str) -> Option<(String, Option<usize>)> {
        let inner = self.inner.get();
        let current_hash = self.hash(path, Showing::Current).ok()?;
        let approved_hash = self.hash(path, Showing::Approved).ok()?;
        let key = format!("{approved_hash}\u{1}{current_hash}");
        if inner
            .change
            .borrow()
            .as_ref()
            .is_none_or(|(seen, _)| *seen != key)
        {
            let approved = self.data(path, Showing::Approved).ok()?;
            let current = self.data(path, Showing::Current).ok()?;
            let change =
                crate::render::change(&approved, &current).unwrap_or_else(|error| Change {
                    description: format!("the paintings could not be compared: {error}"),
                    frame: None,
                });
            *inner.change.borrow_mut() = Some((key, change));
        }
        inner
            .change
            .borrow()
            .as_ref()
            .map(|(_, change)| (change.description.clone(), change.frame))
    }

    pub(crate) fn seek(&self, frame: usize) {
        self.set_frame.set(frame);
        self.set_playing.set(false);
    }

    pub(crate) fn clamp_frame(&self, count: usize) {
        let frame = self.frame.get_untracked().min(count.max(1) - 1);
        if frame != self.frame.get_untracked() {
            self.set_frame.set(frame);
        }
    }

    pub(crate) fn toggle_playback(&self) {
        let inner = self.inner.get();
        let playing = !self.playing.get_untracked();
        self.set_playing.set(playing);
        inner.advanced.set(None);
    }

    pub(crate) fn next_advance(&self, count: usize, rendered: bool) -> Option<Duration> {
        let inner = self.inner.get();
        if !self.playing.get_untracked() || count < 2 {
            inner.advanced.set(None);
            return None;
        }
        if !rendered {
            return None;
        }
        Some(inner.advanced.get().map_or(Duration::ZERO, |advanced| {
            FRAME_INTERVAL.saturating_sub(advanced.elapsed())
        }))
    }

    pub(crate) fn advance(&self, count: usize, rendered: bool) -> Option<Duration> {
        let inner = self.inner.get();
        let wait = self.next_advance(count, rendered)?;
        if !wait.is_zero() {
            return Some(wait);
        }
        self.set_frame.set((self.frame.get_untracked() + 1) % count);
        inner.advanced.set(Some(Instant::now()));
        None
    }

    pub(crate) fn report(&self, scale: Option<f32>, description: Option<String>) {
        if self.scale.get_untracked() != scale {
            self.set_scale.set(scale);
        }
        if self.description.get_untracked() != description {
            self.set_description.set(description);
        }
    }

    pub(crate) fn shown(&self, path: &str, showing: Showing, frame: usize) -> Shown {
        match showing {
            Showing::Approved | Showing::Current => self.single(path, showing, frame),
            Showing::SideBySide => self.side_by_side(path, frame),
            Showing::Difference => self.difference(path, frame),
        }
    }

    fn single(&self, path: &str, showing: Showing, frame: usize) -> Shown {
        let inner = self.inner.get();
        let hash = match self.hash(path, showing) {
            Ok(hash) => hash,
            Err(error) => return Shown::Waiting(error),
        };
        match inner.paintings.borrow_mut().rendered(&hash, frame) {
            Some(Ok(rendered)) => {
                let description = rendered.description.clone();
                Shown::Ready(vec![rendered], description)
            }
            Some(Err(error)) => Shown::Failed(error),
            None => Shown::Waiting(None),
        }
    }

    fn pair(&self, path: &str, frame: usize) -> Result<(Rendered, Rendered), Shown> {
        let inner = self.inner.get();
        let mut sides = Vec::new();
        for wanted in [Showing::Approved, Showing::Current] {
            let hash = match self.hash(path, wanted) {
                Ok(hash) => hash,
                Err(error) => return Err(Shown::Waiting(error)),
            };
            match inner.paintings.borrow_mut().rendered(&hash, frame) {
                Some(Ok(rendered)) => sides.push(rendered),
                Some(Err(error)) => return Err(Shown::Failed(error)),
                None => return Err(Shown::Waiting(None)),
            }
        }
        let current = sides.pop().expect("both sides were rendered");
        let approved = sides.pop().expect("both sides were rendered");
        Ok((approved, current))
    }

    fn side_by_side(&self, path: &str, frame: usize) -> Shown {
        let (approved, current) = match self.pair(path, frame) {
            Ok(pair) => pair,
            Err(shown) => return shown,
        };
        let description = match approved.description == current.description {
            true => current.description.clone(),
            false => format!(
                "{} before, {} now",
                approved.description, current.description
            ),
        };
        Shown::Ready(vec![approved, current], description)
    }

    fn difference(&self, path: &str, frame: usize) -> Shown {
        let inner = self.inner.get();
        let (approved, current) = match self.pair(path, frame) {
            Ok(pair) => pair,
            Err(shown) => return shown,
        };
        let key = self.difference_key(path);
        let count = self.count(path, Showing::Difference);
        let painted = || Ok(crate::render::difference(&approved.image, &current.image));
        let computed = inner
            .paintings
            .borrow_mut()
            .computed(&key, frame, count, painted);
        match computed {
            Ok(rendered) => {
                let description = rendered.description.clone();
                Shown::Ready(vec![rendered], description)
            }
            Err(error) => Shown::Failed(error),
        }
    }
}
