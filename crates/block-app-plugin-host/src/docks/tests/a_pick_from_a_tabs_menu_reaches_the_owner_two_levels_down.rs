use super::*;
use block_plugin_api::DockChildPick;

const TAB: beui::Rect = beui::Rect::from_min_max(beui::pos2(40.0, 0.0), beui::pos2(160.0, 28.0));

fn pick(tab: DockTabPath) -> DockMessage {
    DockMessage::Pick(DockChildPick {
        child: DockChild {
            instance: ROOT,
            child: OUTER_CHILD,
        },
        tab,
        id: "editor.palette".to_owned(),
        anchor: Some(TAB),
    })
}

#[test]
fn a_pick_from_a_tabs_menu_reaches_the_owner_two_levels_down() {
    let mut docks = nested();
    let _ = updates(&mut docks, "outer");
    let _ = updates(&mut docks, "inner");

    docks.heard("shell", vec![pick(path(&[INNER_CHILD], 5))]);
    assert_eq!(
        docks.take_picks("inner"),
        [(INNER, ViewId(5), "editor.palette".to_owned(), Some(TAB))],
        "as a pick of the menu of the tab's view, pointing at the tab"
    );
    assert!(docks.take_picks("outer").is_empty());

    docks.heard("shell", vec![pick(path(&[], 1))]);
    assert_eq!(
        docks.take_picks("outer"),
        [(OUTER, ViewId(1), "editor.palette".to_owned(), Some(TAB))]
    );
    assert!(docks.take_picks("inner").is_empty());
}
