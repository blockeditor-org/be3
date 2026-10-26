use std::time::Instant;

use accesskit::TreeUpdate;

use crate::app::{App, Setup};
use crate::clipboard::{PasteAnswer, PasteKind};
use crate::context::{Context, FrameOutput, WindowOutput};
use crate::file_picker::{FilePickAnswer, FilePickRequest};
use crate::file_saver::{FileSaveAnswer, FileSaveRequest};
use crate::geometry::{Pos2, Vec2};
use crate::renderer::Capture;
use crate::window::{Resize, WindowId, WindowRequest, WindowState};

use super::{Frame, Incoming, PlatformServices};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LayerView {
    pub platform: Vec2,
    pub pixels_per_point: f32,
    pub now: Instant,
}

#[derive(Clone, Default)]
pub struct Shown {
    pub windows: Vec<WindowOutput>,
    pub requests: Vec<WindowRequest>,
}

impl Shown {
    pub fn of(output: &FrameOutput) -> Self {
        Self {
            windows: output.windows.clone(),
            requests: output.window_requests.clone(),
        }
    }

    pub fn window(&self, window: WindowId) -> Option<&WindowOutput> {
        self.windows.iter().find(|shown| shown.id == window)
    }

    pub fn main(&self) -> Option<&WindowOutput> {
        self.window(WindowId::MAIN)
    }
}

pub trait Layer {
    fn attach(&mut self, _context: &Context) {}

    fn setup(&mut self, _setup: &Setup) {}

    fn pace(&mut self) -> Option<Instant> {
        None
    }

    fn now(&mut self, now: Instant) -> Instant {
        now
    }

    fn input(&mut self, _incoming: &mut Vec<Incoming>, _frame: &mut LayerFrame<'_>) {}

    fn output(&mut self, _shown: &mut Shown, _output: &FrameOutput, _frame: &mut LayerFrame<'_>) {}

    fn framed(&mut self, _frame: &mut Frame, _output: &FrameOutput, _layer: &mut LayerFrame<'_>) {}

    fn wants_sight(&self) -> bool {
        false
    }

    fn keyboard_attached(&self) -> Option<bool> {
        None
    }

    fn wants_capture(&self) -> bool {
        false
    }

    fn captured(&mut self, _capture: Option<Capture>) {}

    fn up(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        Some((window, at))
    }

    fn down(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        Some((window, at))
    }

    fn copy(&mut self, text: String, below: &mut dyn PlatformServices) {
        below.copy(text);
    }

    fn copy_image(&mut self, png: Vec<u8>, below: &mut dyn PlatformServices) {
        below.copy_image(png);
    }

    fn paste(
        &mut self,
        kinds: &[PasteKind],
        answer: PasteAnswer,
        below: &mut dyn PlatformServices,
    ) {
        below.paste(kinds, answer);
    }

    fn pick_file(
        &mut self,
        request: FilePickRequest,
        answer: FilePickAnswer,
        below: &mut dyn PlatformServices,
    ) {
        below.pick_file(request, answer);
    }

    fn save_file(
        &mut self,
        request: FileSaveRequest,
        answer: FileSaveAnswer,
        below: &mut dyn PlatformServices,
    ) {
        below.save_file(request, answer);
    }

    fn delivers_paste_shortcut(&self, below: &dyn PlatformServices) -> bool {
        below.delivers_paste_shortcut()
    }

    fn shows_keyboard(&self, below: &dyn PlatformServices) -> bool {
        below.shows_keyboard()
    }

    fn show_window(
        &mut self,
        window: &WindowState,
        shown: &WindowState,
        below: &mut dyn PlatformServices,
    ) {
        below.show_window(window, shown);
    }

    fn resize(&mut self, resize: Resize, below: &mut dyn PlatformServices) -> Result<(), String> {
        below.resize(resize)
    }

    fn set_fullscreen(&mut self, fullscreen: bool, below: &mut dyn PlatformServices) {
        below.set_fullscreen(fullscreen);
    }

    fn publish_accessibility(
        &mut self,
        tree: &mut dyn FnMut() -> TreeUpdate,
        below: &mut dyn PlatformServices,
    ) {
        below.publish_accessibility(tree);
    }
}

pub struct Below<'a> {
    pub(crate) layers: &'a mut [Box<dyn Layer>],
    pub(crate) platform: &'a mut dyn PlatformServices,
}

impl Below<'_> {
    pub fn to_platform(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.layers
            .iter()
            .rev()
            .try_fold((window, at), |(window, at), layer| layer.down(window, at))
    }

    pub fn from_platform(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.layers
            .iter()
            .try_fold((window, at), |(window, at), layer| layer.up(window, at))
    }

    fn beneath(&self) -> Beneath<'_> {
        Beneath {
            layers: &*self.layers,
            platform: &*self.platform,
        }
    }

    fn split(&mut self) -> Option<(&mut Box<dyn Layer>, Below<'_>)> {
        let (top, rest) = self.layers.split_last_mut()?;
        Some((
            top,
            Below {
                layers: rest,
                platform: &mut *self.platform,
            },
        ))
    }
}

impl PlatformServices for Below<'_> {
    fn copy(&mut self, text: String) {
        match self.split() {
            Some((top, mut below)) => top.copy(text, &mut below),
            None => self.platform.copy(text),
        }
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        match self.split() {
            Some((top, mut below)) => top.copy_image(png, &mut below),
            None => self.platform.copy_image(png),
        }
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        match self.split() {
            Some((top, mut below)) => top.paste(kinds, answer, &mut below),
            None => self.platform.paste(kinds, answer),
        }
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        match self.split() {
            Some((top, mut below)) => top.pick_file(request, answer, &mut below),
            None => self.platform.pick_file(request, answer),
        }
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        match self.split() {
            Some((top, mut below)) => top.save_file(request, answer, &mut below),
            None => self.platform.save_file(request, answer),
        }
    }

    fn delivers_paste_shortcut(&self) -> bool {
        self.beneath().delivers_paste_shortcut()
    }

    fn shows_keyboard(&self) -> bool {
        self.beneath().shows_keyboard()
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        match self.split() {
            Some((top, mut below)) => top.show_window(window, shown, &mut below),
            None => self.platform.show_window(window, shown),
        }
    }

    fn resize(&mut self, resize: Resize) -> Result<(), String> {
        match self.split() {
            Some((top, mut below)) => top.resize(resize, &mut below),
            None => self.platform.resize(resize),
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        match self.split() {
            Some((top, mut below)) => top.set_fullscreen(fullscreen, &mut below),
            None => self.platform.set_fullscreen(fullscreen),
        }
    }

    fn publish_accessibility(&mut self, tree: &mut dyn FnMut() -> TreeUpdate) {
        match self.split() {
            Some((top, mut below)) => top.publish_accessibility(tree, &mut below),
            None => self.platform.publish_accessibility(tree),
        }
    }
}

struct Beneath<'a> {
    layers: &'a [Box<dyn Layer>],
    platform: &'a dyn PlatformServices,
}

impl PlatformServices for Beneath<'_> {
    fn delivers_paste_shortcut(&self) -> bool {
        match self.layers.split_last() {
            Some((top, rest)) => top.delivers_paste_shortcut(&Beneath {
                layers: rest,
                platform: self.platform,
            }),
            None => self.platform.delivers_paste_shortcut(),
        }
    }

    fn shows_keyboard(&self) -> bool {
        match self.layers.split_last() {
            Some((top, rest)) => top.shows_keyboard(&Beneath {
                layers: rest,
                platform: self.platform,
            }),
            None => self.platform.shows_keyboard(),
        }
    }
}

pub struct Above<'a> {
    pub(crate) layers: &'a [Box<dyn Layer>],
}

impl Above<'_> {
    pub fn to_app(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.layers
            .iter()
            .try_fold((window, at), |(window, at), layer| layer.up(window, at))
    }

    pub fn from_app(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.layers
            .iter()
            .rev()
            .try_fold((window, at), |(window, at), layer| layer.down(window, at))
    }
}

pub struct LayerFrame<'a> {
    pub view: LayerView,
    pub context: &'a Context,
    pub app: &'a mut dyn App,
    pub below: Below<'a>,
    pub above: Above<'a>,
    pub started: Instant,
    pub(crate) closing: &'a mut bool,
}

impl LayerFrame<'_> {
    pub fn close(&mut self) {
        *self.closing = true;
    }
}

impl<L: Layer> Layer for std::rc::Rc<std::cell::RefCell<L>> {
    fn attach(&mut self, context: &Context) {
        self.borrow_mut().attach(context);
    }

    fn setup(&mut self, setup: &Setup) {
        self.borrow_mut().setup(setup);
    }

    fn pace(&mut self) -> Option<Instant> {
        self.borrow_mut().pace()
    }

    fn now(&mut self, now: Instant) -> Instant {
        self.borrow_mut().now(now)
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, frame: &mut LayerFrame<'_>) {
        self.borrow_mut().input(incoming, frame);
    }

    fn output(&mut self, shown: &mut Shown, output: &FrameOutput, frame: &mut LayerFrame<'_>) {
        self.borrow_mut().output(shown, output, frame);
    }

    fn framed(&mut self, frame: &mut Frame, output: &FrameOutput, layer: &mut LayerFrame<'_>) {
        self.borrow_mut().framed(frame, output, layer);
    }

    fn wants_sight(&self) -> bool {
        self.borrow().wants_sight()
    }

    fn keyboard_attached(&self) -> Option<bool> {
        self.borrow().keyboard_attached()
    }

    fn wants_capture(&self) -> bool {
        self.borrow().wants_capture()
    }

    fn captured(&mut self, capture: Option<Capture>) {
        self.borrow_mut().captured(capture);
    }

    fn up(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.borrow().up(window, at)
    }

    fn down(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.borrow().down(window, at)
    }

    fn copy(&mut self, text: String, below: &mut dyn PlatformServices) {
        self.borrow_mut().copy(text, below);
    }

    fn copy_image(&mut self, png: Vec<u8>, below: &mut dyn PlatformServices) {
        self.borrow_mut().copy_image(png, below);
    }

    fn paste(
        &mut self,
        kinds: &[PasteKind],
        answer: PasteAnswer,
        below: &mut dyn PlatformServices,
    ) {
        self.borrow_mut().paste(kinds, answer, below);
    }

    fn pick_file(
        &mut self,
        request: FilePickRequest,
        answer: FilePickAnswer,
        below: &mut dyn PlatformServices,
    ) {
        self.borrow_mut().pick_file(request, answer, below);
    }

    fn save_file(
        &mut self,
        request: FileSaveRequest,
        answer: FileSaveAnswer,
        below: &mut dyn PlatformServices,
    ) {
        self.borrow_mut().save_file(request, answer, below);
    }

    fn delivers_paste_shortcut(&self, below: &dyn PlatformServices) -> bool {
        self.borrow().delivers_paste_shortcut(below)
    }

    fn shows_keyboard(&self, below: &dyn PlatformServices) -> bool {
        self.borrow().shows_keyboard(below)
    }

    fn show_window(
        &mut self,
        window: &WindowState,
        shown: &WindowState,
        below: &mut dyn PlatformServices,
    ) {
        self.borrow_mut().show_window(window, shown, below);
    }

    fn resize(&mut self, resize: Resize, below: &mut dyn PlatformServices) -> Result<(), String> {
        self.borrow_mut().resize(resize, below)
    }

    fn set_fullscreen(&mut self, fullscreen: bool, below: &mut dyn PlatformServices) {
        self.borrow_mut().set_fullscreen(fullscreen, below);
    }

    fn publish_accessibility(
        &mut self,
        tree: &mut dyn FnMut() -> TreeUpdate,
        below: &mut dyn PlatformServices,
    ) {
        self.borrow_mut().publish_accessibility(tree, below);
    }
}
