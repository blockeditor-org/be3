use super::*;
use beui::dock::{DockAlign, DockBarPane, DockBarPlacement, DockEdge, DockSpecBar};

fn pane(key: &str, entries: Vec<DockSpecEntry>) -> DockSpecNode {
    DockSpecNode::Pane(DockSpecPane {
        key: key.to_owned(),
        entries,
        active: 0,
        vertical: false,
        icons_only: false,
        sidebar: 180.0,
        keep: false,
        area: None,
        bare: false,
        hide_empty_bar: false,
    })
}

/// A bar placed on a pane of a dock joined below this one keeps that pane, named through the
/// tab hosting it, and its slot among the pane's bars, through a saved layout and back.
#[test]
fn a_saved_layout_keeps_a_bars_deeper_pane_and_slot() {
    let host = TabId::new(5);
    let placement = DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center);
    let deeper = DockSpecBar {
        bar: TabId::new(7),
        owner: Some(host),
        placement,
        hidden: false,
        pane: Some(DockBarPane {
            via: vec![host],
            tab: TabId::new(21),
        }),
        slot: Some(1),
    };
    let own = DockSpecBar {
        bar: TabId::new(8),
        owner: None,
        placement,
        hidden: false,
        pane: None,
        slot: Some(0),
    };
    let spec = DockSpec {
        main: Some(pane("main", vec![DockSpecEntry::Tab(host)])),
        bars: vec![own.clone(), deeper.clone()],
        ..DockSpec::default()
    };
    let state = DockState::from_spec(&spec);
    let snapshot = DockingSnapshot::new(state, [host, own.bar, deeper.bar].map(|tab| (tab, tab)));

    let saved = SavedDockLayout::of(&snapshot);
    let text = serde_json::to_string(&saved).expect("a saved layout serializes");
    let back: SavedDockLayout = serde_json::from_str(&text).expect("and reads back");
    assert_eq!(back, saved);
    let bars = back.snapshot().state().spec().bars;
    let restored = |bar: TabId| {
        bars.iter()
            .find(|restored| restored.bar == bar)
            .map(|restored| (restored.pane.clone(), restored.slot))
    };
    assert_eq!(
        restored(deeper.bar),
        Some((deeper.pane.clone(), deeper.slot))
    );
    assert_eq!(restored(own.bar), Some((own.pane.clone(), own.slot)));
}
