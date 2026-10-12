use std::cell::RefCell;
use std::rc::Rc;

use beui::reactive::{Frame, Memo, component, create_memo, view};
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::HostWindowId;
use uuid::Uuid;

pub use crate::be::{content_revision, hold, operate};
pub use crate::host::{
    WakingSender, now, request_repaint, request_repaint_after, wake, waking_channel,
};
pub use crate::notices::{report, report_with};
pub use crate::panic_guard::{contain, contain_task};
pub use crate::platform::utc_offset;
pub use crate::plugin_host::{Access, allow_action, allow_value, publish, take_actions, watched};
pub use crate::{APP_ID, commit};

pub fn content(block: Uuid) -> Option<Vec<u8>> {
    crate::be::content(block).map(|content| content.bytes)
}

pub struct App<'a> {
    app: &'a mut crate::BlockApp,
}

impl<'a> App<'a> {
    pub(crate) fn new(app: &'a mut crate::BlockApp) -> Self {
        Self { app }
    }

    pub fn settings_block(&mut self, content_type: Uuid) -> Option<Uuid> {
        self.app.settings_block(content_type)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cached_copy(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.app
            .app_state
            .cached_copy(key)
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cache_copy(&self, key: &str, content: &[u8]) -> Result<(), String> {
        self.app
            .app_state
            .cache_copy(key, content)
            .map_err(|error| error.to_string())
    }
}

pub trait Extension {
    fn view(&mut self) -> NodeId;

    fn setup(&mut self, _setup: &Setup, _app: &mut App) {}

    fn renderer_replaced(&mut self, _setup: &Setup) {}

    fn before(&mut self, _context: &Context, _rect: Rect, _document: &mut Document) {}

    fn frame(&mut self, _context: &Context, _document: &mut Document, _app: &mut App) {}

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

#[derive(Default)]
struct Hooks {
    window: Option<WindowView>,
}

thread_local! {
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
}

pub fn show_windows_with(view: impl Fn(HostWindowId, Memo<Vec<Rect>>) -> NodeId + 'static) {
    HOOKS.with(|hooks| hooks.borrow_mut().window = Some(Rc::new(view)));
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
