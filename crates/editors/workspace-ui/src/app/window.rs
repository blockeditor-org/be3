use block_editor_beui::beui::reactive::{component, view};
use block_editor_beui::beui::styled::WINDOW_CHROME;
use block_editor_beui::beui::unstyled::TabId;
use block_editor_beui::beui::{NodeId, Rect, pos2, vec2};
use block_editor_beui::{ChildMode, Editor, HostWindow, HostWindowId, Subregion, SubregionContent};

const WINDOW_TABS: u64 = 1 << 41;
const DIALOG_ORIGIN: f32 = 80.0;

pub(crate) fn window_tab(window: HostWindowId) -> TabId {
    TabId::new(WINDOW_TABS + window.0)
}

pub(crate) fn tab_window(tab: TabId) -> Option<HostWindowId> {
    tab.value().checked_sub(WINDOW_TABS).map(HostWindowId)
}

pub(crate) fn dialog_window(window: &HostWindow) -> Rect {
    Rect::from_min_size(
        pos2(DIALOG_ORIGIN, DIALOG_ORIGIN),
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
pub(crate) fn WindowPanel(editor: Editor, window: HostWindowId) -> NodeId {
    view! {
        <Subregion
            editor={editor}
            placed={Some(SubregionContent::Window(window))}
            mode=ChildMode::Live
            @test_id={"workspace.window"}
        />
    }
}
