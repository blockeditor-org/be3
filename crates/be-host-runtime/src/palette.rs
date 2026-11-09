//! What a host that draws the command palette asks of the runtime: whether it draws one
//! (`open-block.host-palette`), the palettes apps ask it to open (`shell.open-palette`), each
//! view's actions and running one.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use be_sdk_api::{DescribedAction, EditorInstanceId, PaletteMode, ViewId};

static DRAWS_PALETTE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static ASKED: RefCell<Vec<PaletteAsk>> = const { RefCell::new(Vec::new()) };
}

/// An app's ask for the host's palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteAsk {
    pub app: String,
    pub instance: EditorInstanceId,
    pub mode: PaletteMode,
}

/// Whether the host draws the command palette, which every editor opened from now on hears.
pub fn draw_palette(draws: bool) {
    DRAWS_PALETTE.store(draws, Ordering::Relaxed);
}

pub(crate) fn host_draws_palette() -> bool {
    DRAWS_PALETTE.load(Ordering::Relaxed)
}

pub(crate) fn ask(app: &str, instance: EditorInstanceId, mode: PaletteMode) {
    ASKED.with(|asked| {
        asked.borrow_mut().push(PaletteAsk {
            app: app.to_owned(),
            instance,
            mode,
        });
    });
}

pub fn take_palette_asks() -> Vec<PaletteAsk> {
    ASKED.with(|asked| std::mem::take(&mut *asked.borrow_mut()))
}

/// The actions every view of the instance last described, those of `first` first, each with
/// the view that runs it.
pub fn instance_actions(
    app_id: &str,
    instance: EditorInstanceId,
    first: ViewId,
) -> Vec<(ViewId, DescribedAction)> {
    crate::runtime::with(app_id, |runtime| {
        runtime.instances.instance_actions(instance, first)
    })
    .unwrap_or_default()
}

/// Runs one of the view's actions by id (`app.run-action`), on the screen that shows the view.
pub fn run_view_action(app_id: &str, instance: EditorInstanceId, view: ViewId, id: &str) {
    crate::runtime::with(app_id, |runtime| {
        if let Some((screen, _)) = runtime.instances.view_actions(instance, view) {
            runtime.run_action(screen, id.to_owned());
        }
    });
}
