use super::*;

#[test]
fn a_tab_the_root_closes_is_closed_at_its_owner() {
    let mut docks = nested();
    docks.heard(
        "shell",
        vec![DockMessage::Close(DockChildClose {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            tab: path(&[INNER_CHILD], 5),
        })],
    );
    assert_eq!(
        updates(&mut docks, "inner"),
        [(
            INNER,
            DockUpdate::Close(DockClosedTab {
                view: ViewId::MAIN,
                tab: path(&[], 5),
            })
        )]
    );
    assert!(updates(&mut docks, "outer").is_empty());
}
