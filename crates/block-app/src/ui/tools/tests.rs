use super::*;
use block_plugin_api::{PaneInfo, PaneItem};

mod a_pane_layout_sent_before_the_last_arrangement_is_ignored;
mod a_pane_moved_out_of_the_workspace_stays_out_when_the_plugin_rearranges;
mod the_workspace_tab_becomes_a_pinned_group_of_the_plugins_panes;

fn layout(panes: &[u64], arrangement: u64) -> PaneLayout {
    let mut items = vec![PaneItem::Tabs {
        count: panes.len() as u32,
        active: 0,
    }];
    items.extend(panes.iter().map(|pane| PaneItem::Pane(PaneId(*pane))));
    PaneLayout {
        panes: panes
            .iter()
            .map(|pane| PaneInfo {
                pane: PaneId(*pane),
                title: format!("Pane {pane}"),
                closable: true,
            })
            .collect(),
        tree: PaneTree { items },
        arrangement,
    }
}
