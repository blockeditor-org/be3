use std::cell::RefCell;
use std::rc::Rc;

use be_block::{DisplaySettings, InputSettings};
use beui::reactive::{Frame, Memo, component, create_memo, view};
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::HostWindowId;

pub use crate::display::{Screen, set_screens};
pub use crate::host::{
    WakingSender, now, request_repaint, request_repaint_after, wake, waking_channel,
};
pub use crate::notices::{report, report_with};
pub use crate::panic_guard::{contain, contain_task};
pub use crate::platform::utc_offset;
pub use crate::plugin_host::{Access, allow_action, allow_value, publish, take_actions, watched};
pub use crate::{APP_ID, commit};

pub trait Extension {
    fn view(&mut self) -> NodeId;

    fn setup(&mut self, _setup: &Setup) {}

    fn renderer_replaced(&mut self, _setup: &Setup) {}

    fn before(&mut self, _context: &Context, _rect: Rect, _document: &mut Document) {}

    fn frame(&mut self, _context: &Context, _document: &mut Document) {}

    fn after(&mut self, _context: &Context, _document: &mut Document) {}

    fn exiting(&mut self) {}

    fn ends_on_crash(&self) -> bool {
        false
    }
}

pub(crate) struct NoExtension;

impl Extension for NoExtension {
    fn view(&mut self) -> NodeId {
        view! {
            <Frame />
        }
    }
}

type WindowView = Rc<dyn Fn(HostWindowId, Memo<Vec<Rect>>) -> NodeId>;
type Apply<T> = Rc<dyn Fn(&T)>;

#[derive(Default)]
struct Hooks {
    window: Option<WindowView>,
    display: Option<Apply<DisplaySettings>>,
    input: Option<Apply<InputSettings>>,
}

thread_local! {
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
}

pub fn show_windows_with(view: impl Fn(HostWindowId, Memo<Vec<Rect>>) -> NodeId + 'static) {
    HOOKS.with(|hooks| hooks.borrow_mut().window = Some(Rc::new(view)));
}

pub fn apply_display_with(apply: impl Fn(&DisplaySettings) + 'static) {
    HOOKS.with(|hooks| hooks.borrow_mut().display = Some(Rc::new(apply)));
}

pub fn apply_input_with(apply: impl Fn(&InputSettings) + 'static) {
    HOOKS.with(|hooks| hooks.borrow_mut().input = Some(Rc::new(apply)));
}

pub(crate) fn apply_display(settings: &DisplaySettings) {
    if let Some(apply) = HOOKS.with(|hooks| hooks.borrow().display.clone()) {
        apply(settings);
    }
}

pub(crate) fn apply_input(settings: &InputSettings) {
    if let Some(apply) = HOOKS.with(|hooks| hooks.borrow().input.clone()) {
        apply(settings);
    }
}

pub fn close_app_menu() {
    crate::ui::send(crate::ui::UiCommand::AppMenu(false));
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    let occluders = crate::compositor::region::occlusion();
    let occluders = create_memo(move || {
        occluders
            .as_ref()
            .map(|occluders| occluders.get())
            .unwrap_or_default()
    });
    match HOOKS.with(|hooks| hooks.borrow().window.clone()) {
        Some(view) => view(window, occluders),
        None => view! {
            <Frame />
        },
    }
}
