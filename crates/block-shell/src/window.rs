use block_editor_beui::beui::reactive::{
    Memo, clone, component, create_effect, create_memo, untrack, view,
};
use block_editor_beui::beui::styled::WINDOW_CHROME;
use block_editor_beui::beui::unstyled::{TabId, use_dock_tab};
use block_editor_beui::beui::{NodeId, Rect, pos2, vec2};
use block_editor_beui::{ChildMode, Editor, HostWindow, HostWindowId, Subregion, SubregionContent};

const WINDOW_TABS: u64 = 1 << 41;
const DIALOG_ORIGIN: f32 = 80.0;
const DIALOG_CASCADE: f32 = 32.0;
const DIALOG_CASCADES: u64 = 8;

pub(crate) fn window_tab(window: HostWindowId) -> TabId {
    TabId::new(WINDOW_TABS + window.0)
}

pub(crate) fn tab_window(tab: TabId) -> Option<HostWindowId> {
    tab.value().checked_sub(WINDOW_TABS).map(HostWindowId)
}

pub(crate) fn dialog_window(window: &HostWindow) -> Rect {
    let step = (window.id.0 % DIALOG_CASCADES) as f32 * DIALOG_CASCADE;
    Rect::from_min_size(
        pos2(DIALOG_ORIGIN + step, DIALOG_ORIGIN + step),
        vec2(window.size.width, window.size.height) + WINDOW_CHROME,
    )
}

pub(crate) fn window_title(window: &HostWindow) -> String {
    match (window.title.is_empty(), window.app_id.is_empty()) {
        (false, _) => window.title.clone(),
        (true, false) => window.app_id.clone(),
        (true, true) => "Window".to_owned(),
    }
}

#[component]
pub(crate) fn WindowPanel(
    editor: Editor,
    window: HostWindowId,
    fullscreen: Memo<Option<Rect>>,
) -> NodeId {
    if let Some(tab) = use_dock_tab() {
        create_effect(clone!(tab fullscreen -> move || match fullscreen.get() {
            Some(area) => tab.enter_fullscreen(Some(area)),
            None => untrack(|| tab.leave_fullscreen()),
        }));
        let shown = create_memo(clone!(tab -> move || tab.fullscreen()));
        let host = editor.host().clone();
        create_effect(move || {
            let shown = shown.get();
            if shown != fullscreen.get_untracked().is_some() {
                host.fullscreen_window(window, shown);
            }
        });
    }
    view! {
        <Subregion
            editor={editor}
            placed={Some(SubregionContent::Window(window))}
            mode=ChildMode::Live
            @test_id={"workspace.window"}
        />
    }
}
