use block_editor_beui::beui::icons::{ICON_EXTENSION, ICON_HISTORY, ICON_LAYERS, ICON_SPEED};
use block_editor_beui::beui::reactive::{component, view};
use block_editor_beui::beui::unstyled::TabId;
use block_editor_beui::beui::{NodeId, Rect, pos2, vec2};
use block_editor_beui::{ChildMode, Editor, HostPanel, Subregion, SubregionContent};

const PANEL_TABS: u64 = 1 << 40;

pub(crate) fn panel_tab(panel: HostPanel) -> TabId {
    let index = HostPanel::ALL
        .iter()
        .position(|listed| *listed == panel)
        .unwrap_or_default();
    TabId::new(PANEL_TABS + index as u64)
}

pub(crate) fn tab_panel(tab: TabId) -> Option<HostPanel> {
    let index = tab.value().checked_sub(PANEL_TABS)?;
    HostPanel::ALL.get(usize::try_from(index).ok()?).copied()
}

pub(crate) fn panel_icon(panel: HostPanel) -> &'static str {
    match panel {
        HostPanel::BlockStack => ICON_LAYERS,
        HostPanel::Performance => ICON_SPEED,
        HostPanel::Plugins => ICON_EXTENSION,
        HostPanel::Version => ICON_HISTORY,
    }
}

pub(crate) fn panel_window(panel: HostPanel) -> Rect {
    let (position, size) = match panel {
        HostPanel::BlockStack => (pos2(60.0, 60.0), vec2(760.0, 600.0)),
        HostPanel::Performance => (pos2(120.0, 100.0), vec2(520.0, 420.0)),
        HostPanel::Plugins => (pos2(150.0, 90.0), vec2(560.0, 440.0)),
        HostPanel::Version => (pos2(180.0, 110.0), vec2(640.0, 480.0)),
    };
    Rect::from_min_size(position, size)
}

#[component]
pub(crate) fn HostPanelView(editor: Editor, panel: HostPanel) -> NodeId {
    view! {
        <Subregion
            editor={editor}
            placed={Some(SubregionContent::Host(panel))}
            mode=ChildMode::Live
            @test_id={"workspace.host-panel"}
        />
    }
}
