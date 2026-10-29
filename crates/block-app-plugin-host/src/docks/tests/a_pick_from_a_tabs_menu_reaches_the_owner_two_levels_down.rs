use super::*;
use block_plugin_api::DockChildPick;

fn pick(tab: DockTabPath) -> DockMessage {
    DockMessage::Pick(DockChildPick {
        child: DockChild {
            instance: ROOT,
            child: OUTER_CHILD,
        },
        tab,
        id: "editor.palette".to_owned(),
    })
}

#[test]
fn a_pick_from_a_tabs_menu_reaches_the_owner_two_levels_down() {
    let mut docks = nested();
    let _ = updates(&mut docks, "outer");
    let _ = updates(&mut docks, "inner");

    docks.heard("shell", vec![pick(path(&[INNER_CHILD], 5))]);
    assert_eq!(
        updates(&mut docks, "inner"),
        [(
            INNER,
            DockUpdate::Pick(DockPickedTab {
                view: ViewId::MAIN,
                tab: path(&[], 5),
                id: "editor.palette".to_owned(),
            })
        )]
    );
    assert!(updates(&mut docks, "outer").is_empty());

    docks.heard("shell", vec![pick(path(&[], 1))]);
    assert_eq!(
        updates(&mut docks, "outer"),
        [(
            OUTER,
            DockUpdate::Pick(DockPickedTab {
                view: ViewId::MAIN,
                tab: path(&[], 1),
                id: "editor.palette".to_owned(),
            })
        )]
    );
    assert!(updates(&mut docks, "inner").is_empty());
}
